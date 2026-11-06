use mysql_async::consts::ColumnType as T;
use mysql_async::prelude::*;
use mysql_async::{Column, Conn, Value};
use serde::Serialize;
use std::time::Instant;

const CELL_CAP: usize = 64 * 1024;
const HEX_CAP: usize = 1024;
pub const MAX_ROWS_LIMIT: usize = 10_000;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub truncated: bool,
    pub affected: Option<u64>,
    pub last_insert_id: Option<String>,
    pub current_db: Option<String>,
    pub elapsed_ms: u64,
    pub notice: Option<String>,
}

pub fn quote_ident(s: &str) -> String {
    format!("`{}`", s.replace('`', "``"))
}

fn is_binary(col: &Column) -> bool {
    col.character_set() == 63
        && matches!(
            col.column_type(),
            T::MYSQL_TYPE_BLOB
                | T::MYSQL_TYPE_TINY_BLOB
                | T::MYSQL_TYPE_MEDIUM_BLOB
                | T::MYSQL_TYPE_LONG_BLOB
                | T::MYSQL_TYPE_STRING
                | T::MYSQL_TYPE_VAR_STRING
                | T::MYSQL_TYPE_VARCHAR
                | T::MYSQL_TYPE_BIT
                | T::MYSQL_TYPE_GEOMETRY
        )
}

fn cap(mut s: String) -> String {
    if s.len() > CELL_CAP {
        let mut end = CELL_CAP;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

pub fn render(v: Value, binary: bool) -> Option<String> {
    match v {
        Value::NULL => None,
        Value::Bytes(b) if binary => {
            let mut s = String::from("0x");
            for x in b.iter().take(HEX_CAP) {
                s.push_str(&format!("{x:02X}"));
            }
            if b.len() > HEX_CAP {
                s.push_str(&format!("… ({} bytes)", b.len()));
            }
            Some(s)
        }
        Value::Bytes(b) => Some(cap(String::from_utf8_lossy(&b).into_owned())),
        Value::Int(i) => Some(i.to_string()),
        Value::UInt(u) => Some(u.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Double(f) => Some(f.to_string()),
        other => Some(other.as_sql(true).trim_matches('\'').to_string()),
    }
}

/// Makes sure the session connection is usable: pings it and reconnects if it was dropped
/// (server wait_timeout, NAT/tunnel idle drop). Returns a notice when the session was replaced.
pub async fn ensure_session(slot: &mut Option<Conn>, opts: &mysql_async::Opts) -> mysql_async::Result<Option<String>> {
    let lost = match slot.as_mut() {
        Some(c) => c.ping().await.is_err(),
        None => true,
    };
    if !lost {
        return Ok(None);
    }
    *slot = None;
    *slot = Some(Conn::new(opts.clone()).await?);
    Ok(Some("reconnected: previous session was closed (transaction / session variables were reset)".into()))
}

/// Runs `sql` on the session connection. Keeps the first result set (up to `max_rows`),
/// drains everything else so the connection stays reusable.
pub async fn exec(conn: &mut Conn, db: Option<&str>, sql: &str, max_rows: usize) -> mysql_async::Result<QueryResult> {
    if let Some(db) = db {
        let cur: Option<String> = conn.query_first("SELECT DATABASE()").await?.flatten();
        if cur.as_deref() != Some(db) {
            conn.query_drop(format!("USE {}", quote_ident(db))).await?;
        }
    }
    let max_rows = max_rows.clamp(1, MAX_ROWS_LIMIT);
    let start = Instant::now();
    let mut result = conn.query_iter(sql).await?;
    let cols = result.columns().map(|c| c.to_vec()).unwrap_or_default();
    let binary: Vec<bool> = cols.iter().map(is_binary).collect();
    let mut rows = Vec::new();
    let mut truncated = false;
    // ponytail: rows past the cap are still streamed from the server and discarded; cancel if that is slow.
    while let Some(row) = result.next().await? {
        if rows.len() >= max_rows {
            truncated = true;
            continue;
        }
        rows.push(row.unwrap().into_iter().zip(&binary).map(|(v, b)| render(v, *b)).collect());
    }
    let has_cols = !cols.is_empty();
    let affected = (!has_cols).then(|| result.affected_rows());
    let last_insert_id = result.last_insert_id().filter(|id| *id > 0).map(|id| id.to_string());
    result.drop_result().await?; // remaining result sets (e.g. CALL)
    let elapsed_ms = start.elapsed().as_millis() as u64;
    let current_db: Option<String> = conn.query_first("SELECT DATABASE()").await?.flatten();
    Ok(QueryResult {
        columns: cols.iter().map(|c| c.name_str().into_owned()).collect(),
        rows,
        truncated,
        affected,
        last_insert_id,
        current_db,
        elapsed_ms,
        notice: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_caps() {
        assert_eq!(quote_ident("a`b"), "`a``b`");
        assert_eq!(render(Value::NULL, false), None);
        assert_eq!(render(Value::Bytes(vec![0xde, 0xad]), true).unwrap(), "0xDEAD");
        let big = render(Value::Bytes(vec![b'a'; CELL_CAP + 10]), false).unwrap();
        assert!(big.ends_with('…') && big.len() <= CELL_CAP + 3);
    }

    /// Needs local MySQL: DBPANE_TEST_URL=mysql://root:test@127.0.0.1:33307 cargo test -- --ignored
    #[tokio::test]
    #[ignore]
    async fn live_mysql() {
        let url = std::env::var("DBPANE_TEST_URL").unwrap();
        let mut c = Conn::new(mysql_async::Opts::from_url(&url).unwrap()).await.unwrap();
        // cap + truncation, rows past the cap leave the connection usable
        let r = exec(&mut c, None, "SELECT 1 a, 1 a UNION ALL SELECT 2,2 UNION ALL SELECT 3,3", 2).await.unwrap();
        assert_eq!(r.columns, vec!["a", "a"]);
        assert_eq!(r.rows.len(), 2);
        assert!(r.truncated);
        // binary vs text vs numeric/date
        let r = exec(&mut c, None, "SELECT X'DEAD', 'héllo', 42, DATE('2024-01-02'), NULL", 10).await.unwrap();
        assert_eq!(r.rows[0], vec![Some("0xDEAD".into()), Some("héllo".into()), Some("42".into()), Some("2024-01-02".into()), None]);
        // db switch + user-typed USE is reported back
        let r = exec(&mut c, Some("mysql"), "SELECT 1", 10).await.unwrap();
        assert_eq!(r.current_db.as_deref(), Some("mysql"));
        let r = exec(&mut c, None, "USE information_schema", 10).await.unwrap();
        assert_eq!(r.current_db.as_deref(), Some("information_schema"));
        // session state persists (temporary table + transaction)
        exec(&mut c, Some("mysql"), "CREATE TEMPORARY TABLE t (x INT)", 10).await.unwrap();
        let r = exec(&mut c, None, "INSERT INTO t VALUES (1),(2)", 10).await.unwrap();
        assert_eq!(r.affected, Some(2));
        let r = exec(&mut c, None, "SELECT COUNT(*) FROM t", 10).await.unwrap();
        assert_eq!(r.rows[0][0].as_deref(), Some("2"));
        // dropped session is detected and replaced
        let opts = mysql_async::Opts::from_url(&url).unwrap();
        let mut slot = Some(Conn::new(opts.clone()).await.unwrap());
        assert_eq!(ensure_session(&mut slot, &opts).await.unwrap(), None);
        let tid = slot.as_ref().unwrap().id();
        c.query_drop(format!("KILL {tid}")).await.unwrap();
        assert!(ensure_session(&mut slot, &opts).await.unwrap().is_some());
        assert_ne!(slot.as_ref().unwrap().id(), tid);
        let r = exec(slot.as_mut().unwrap(), None, "SELECT 5", 10).await.unwrap();
        assert_eq!(r.rows[0][0].as_deref(), Some("5"));
        // error then reuse
        assert!(exec(&mut c, None, "SELEC nope", 10).await.is_err());
        let r = exec(&mut c, None, "SELECT 7", 10).await.unwrap();
        assert_eq!(r.rows[0][0].as_deref(), Some("7"));
    }
}

