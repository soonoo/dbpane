#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;

use mysql_async::prelude::*;
use mysql_async::{Conn, Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

const KEYCHAIN_SERVICE: &str = "dev.soonoo.dbpane";

#[derive(Serialize, Deserialize, Clone)]
struct ConnConfig {
    id: String,
    name: String,
    host: String,
    port: u16,
    user: String,
    database: Option<String>,
}

struct Open {
    opts: Opts,
    pool: Pool, // metadata + KILL QUERY
    session: tokio::sync::Mutex<Option<Conn>>,
    /// Some(0) = run starting, Some(tid) = query running on that thread id.
    running: Mutex<Option<u32>>,
}

struct AppState {
    cfg_path: PathBuf,
    open: Mutex<HashMap<String, Arc<Open>>>,
}

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn load(st: &AppState) -> Vec<ConnConfig> {
    std::fs::read(&st.cfg_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn store(st: &AppState, list: &[ConnConfig]) -> R<()> {
    if let Some(dir) = st.cfg_path.parent() {
        std::fs::create_dir_all(dir).map_err(e)?;
    }
    std::fs::write(&st.cfg_path, serde_json::to_vec_pretty(list).map_err(e)?).map_err(e)
}

fn keychain(id: &str) -> R<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, id).map_err(e)
}

fn stored_password(id: &str) -> R<Option<String>> {
    match keychain(id)?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(e(err)),
    }
}

fn opts(cfg: &ConnConfig, password: Option<String>) -> Opts {
    OptsBuilder::default()
        .ip_or_hostname(cfg.host.clone())
        .tcp_port(cfg.port)
        .user(Some(cfg.user.clone()))
        .pass(password)
        .db_name(cfg.database.clone().filter(|d| !d.is_empty()))
        .into()
}

fn get_open(st: &AppState, id: &str) -> R<Arc<Open>> {
    st.open.lock().unwrap().get(id).cloned().ok_or_else(|| "connection is not open".into())
}

async fn close(st: &AppState, id: &str) {
    let o = st.open.lock().unwrap().remove(id);
    if let Some(o) = o {
        if let Some(c) = o.session.lock().await.take() {
            let _ = c.disconnect().await;
        }
        let _ = o.pool.clone().disconnect().await;
    }
}

#[tauri::command]
fn list_connections(st: State<AppState>) -> Vec<ConnConfig> {
    load(&st)
}

#[tauri::command]
async fn save_connection(st: State<'_, AppState>, mut cfg: ConnConfig, password: Option<String>) -> R<ConnConfig> {
    if cfg.id.is_empty() {
        cfg.id = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    }
    match password.as_deref() {
        None => {}
        Some("") => match keychain(&cfg.id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => return Err(e(err)),
        },
        Some(p) => keychain(&cfg.id)?.set_password(p).map_err(e)?,
    }
    let mut list = load(&st);
    match list.iter_mut().find(|c| c.id == cfg.id) {
        Some(c) => *c = cfg.clone(),
        None => list.push(cfg.clone()),
    }
    store(&st, &list)?;
    close(&st, &cfg.id).await; // settings changed: reopen on next use
    Ok(cfg)
}

#[tauri::command]
async fn delete_connection(st: State<'_, AppState>, id: String) -> R<()> {
    close(&st, &id).await;
    let list: Vec<_> = load(&st).into_iter().filter(|c| c.id != id).collect();
    store(&st, &list)?;
    match keychain(&id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(e(err)),
    }
}

#[tauri::command]
async fn test_connection(cfg: ConnConfig, password: Option<String>) -> R<String> {
    let pw = match password {
        Some(p) => Some(p).filter(|p| !p.is_empty()),
        None => stored_password(&cfg.id)?,
    };
    let mut c = Conn::new(opts(&cfg, pw)).await.map_err(e)?;
    let v: Option<String> = c.query_first("SELECT VERSION()").await.map_err(e)?;
    let _ = c.disconnect().await;
    Ok(v.unwrap_or_default())
}

#[tauri::command]
async fn open_connection(st: State<'_, AppState>, id: String) -> R<()> {
    if st.open.lock().unwrap().contains_key(&id) {
        return Ok(());
    }
    let cfg = load(&st).into_iter().find(|c| c.id == id).ok_or("unknown connection")?;
    let o = opts(&cfg, stored_password(&id)?);
    let pool_opts = PoolOpts::default().with_constraints(PoolConstraints::new(0, 2).unwrap());
    let pool = Pool::new(OptsBuilder::from_opts(o.clone()).pool_opts(pool_opts));
    pool.get_conn().await.map_err(e)?; // fail fast on bad credentials
    let session = Conn::new(o.clone()).await.map_err(e)?;
    st.open.lock().unwrap().insert(
        id,
        Arc::new(Open { opts: o, pool, session: tokio::sync::Mutex::new(Some(session)), running: Mutex::new(None) }),
    );
    Ok(())
}

#[tauri::command]
async fn close_connection(st: State<'_, AppState>, id: String) -> R<()> {
    close(&st, &id).await;
    Ok(())
}

#[tauri::command]
async fn list_databases(st: State<'_, AppState>, id: String) -> R<Vec<String>> {
    let pool = get_open(&st, &id)?.pool.clone();
    pool.get_conn().await.map_err(e)?.query("SHOW DATABASES").await.map_err(e)
}

#[derive(Serialize)]
struct TableInfo {
    name: String,
    kind: String,
}

#[tauri::command]
async fn list_tables(st: State<'_, AppState>, id: String, db: String) -> R<Vec<TableInfo>> {
    let pool = get_open(&st, &id)?.pool.clone();
    let rows: Vec<(String, String)> = pool
        .get_conn()
        .await
        .map_err(e)?
        .exec(
            "SELECT TABLE_NAME, IF(TABLE_TYPE='VIEW','VIEW','TABLE') FROM information_schema.TABLES WHERE TABLE_SCHEMA=? ORDER BY TABLE_NAME",
            (db,),
        )
        .await
        .map_err(e)?;
    Ok(rows.into_iter().map(|(name, kind)| TableInfo { name, kind }).collect())
}

#[derive(Serialize)]
struct ColumnInfo {
    name: String,
    r#type: String,
    nullable: bool,
    key: String,
    default: Option<String>,
    extra: String,
}

#[tauri::command]
async fn list_columns(st: State<'_, AppState>, id: String, db: String, table: String) -> R<Vec<ColumnInfo>> {
    let pool = get_open(&st, &id)?.pool.clone();
    let rows: Vec<(String, String, String, String, Option<String>, String)> = pool
        .get_conn()
        .await
        .map_err(e)?
        .exec(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, EXTRA FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION",
            (db, table),
        )
        .await
        .map_err(e)?;
    Ok(rows
        .into_iter()
        .map(|(name, t, n, key, default, extra)| ColumnInfo { name, r#type: t, nullable: n == "YES", key, default, extra })
        .collect())
}

/// table -> column names, for editor autocomplete.
#[tauri::command]
async fn schema_columns(st: State<'_, AppState>, id: String, db: String) -> R<HashMap<String, Vec<String>>> {
    let pool = get_open(&st, &id)?.pool.clone();
    let rows: Vec<(String, String)> = pool
        .get_conn()
        .await
        .map_err(e)?
        .exec(
            "SELECT TABLE_NAME, COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? ORDER BY TABLE_NAME, ORDINAL_POSITION",
            (db,),
        )
        .await
        .map_err(e)?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (t, c) in rows {
        map.entry(t).or_default().push(c);
    }
    Ok(map)
}

struct RunGuard<'a>(&'a Mutex<Option<u32>>);
impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = None;
    }
}

#[tauri::command]
async fn run_query(st: State<'_, AppState>, id: String, db: Option<String>, sql: String, max_rows: usize) -> R<db::QueryResult> {
    let o = get_open(&st, &id)?;
    {
        let mut r = o.running.lock().unwrap();
        if r.is_some() {
            return Err("a query is already running on this connection".into());
        }
        *r = Some(0);
    }
    let _guard = RunGuard(&o.running);
    let mut session = o.session.lock().await;
    let notice = db::ensure_session(&mut session, &o.opts).await.map_err(e)?;
    let conn = session.as_mut().unwrap();
    *o.running.lock().unwrap() = Some(conn.id());
    match db::exec(conn, db.as_deref(), &sql, max_rows).await {
        Ok(mut res) => {
            res.notice = notice;
            Ok(res)
        }
        Err(err) => {
            if !matches!(err, mysql_async::Error::Server(_)) {
                *session = None; // broken protocol/io: reconnect on next run
            }
            Err(e(err))
        }
    }
}

#[tauri::command]
async fn cancel_query(st: State<'_, AppState>, id: String) -> R<bool> {
    let o = get_open(&st, &id)?;
    let tid = *o.running.lock().unwrap();
    match tid {
        Some(t) if t != 0 => {
            let mut c = o.pool.get_conn().await.map_err(e)?;
            c.query_drop(format!("KILL QUERY {t}")).await.map_err(e)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let cfg_path = app.path().app_config_dir()?.join("connections.json");
            app.manage(AppState { cfg_path, open: Mutex::new(HashMap::new()) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_connections,
            save_connection,
            delete_connection,
            test_connection,
            open_connection,
            close_connection,
            list_databases,
            list_tables,
            list_columns,
            schema_columns,
            run_query,
            cancel_query
        ])
        .run(tauri::generate_context!())
        .expect("error while running dbpane");
}
