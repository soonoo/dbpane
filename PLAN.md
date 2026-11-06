# dbpane — lightweight MySQL GUI (DataGrip alternative)

## Goal
Personal desktop DB client for macOS. MySQL only for now. Low RAM/CPU (target < 150MB idle).

## Success criteria
- Save/edit/delete connections; password stored in macOS Keychain (never in plain config).
- Left sidebar tree: connection → database → table/view → column (type, nullable, key). Lazy loaded on expand.
- Right: SQL editor (MySQL highlighting, schema-aware autocomplete). Cmd+Enter runs selection, else statement under cursor.
- Bottom: Results tab (virtualized grid) + Log tab (time, SQL, duration, rows/affected, error).
- Sidebar | main and editor / bottom boundaries resizable by mouse drag; sizes persisted.
- Running query can be cancelled.
- Runs locally via `pnpm tauri dev` against local MySQL (docker, 127.0.0.1:33307).

## Stack
- Tauri 2 (Rust) shell + backend.
- Frontend: Svelte 5 + Vite + TS (plain SPA, no SvelteKit).
- Editor: CodeMirror 6 + `@codemirror/lang-sql` (MySQL dialect, schema completion).
- Grid: hand-rolled virtual scroll (fixed row height, render visible rows only). No grid library.
- Splitters: CSS Grid + pointer events (~40 lines). No library.
- Rust DB: `mysql_async` (dynamic `Value` enum suits arbitrary result sets). `keyring` crate for Keychain. `serde_json` config in app config dir.

## Backend contract (Tauri commands; frontend depends on these exactly)
```
type ConnConfig = { id: string, name: string, host: string, port: number, user: string, database?: string }
list_connections() -> ConnConfig[]
save_connection(cfg: ConnConfig, password?: string) -> ConnConfig   // password undefined = keep existing
delete_connection(id) -> void
test_connection(cfg, password?) -> string                            // server version
open_connection(id) -> void                                          // creates pool for id
close_connection(id) -> void
list_databases(id) -> string[]
list_tables(id, db) -> { name: string, kind: "TABLE"|"VIEW" }[]
list_columns(id, db, table) -> { name, type, nullable: bool, key: string, default?: string, extra: string }[]
run_query(id, db: string|null, sql, maxRows: number) -> QueryResult
cancel_query(id) -> void
type QueryResult = {
  columns: string[], rows: (string|null)[][],       // all values rendered as strings, NULL = null, binary = hex/"<binary N bytes>"
  truncated: boolean,                                 // more rows existed than maxRows
  affected: number|null, lastInsertId: string|null, currentDb: string|null,
  elapsedMs: number
}
Errors: commands return Err(String) -> rejected promise with message.
```

## Key decisions
- One dedicated session connection per open connection for queries (so `USE`, session vars, transactions persist across runs), plus pool for metadata/cancel.
- Before each run, if `db` differs from session's current db, issue `USE \`db\``.
- Row cap (review #1): keep first `maxRows` (backend clamps 1..10000), stream & discard the rest, `truncated` = a row beyond cap was seen. No KILL for capping; user cancels if draining is slow.
- Cancel (review #2): backend tracks `running: Option<thread_id>` outside the session lock. `cancel_query` kills only when a run is in flight, via a small side pool (max 2). `run_query` rejects if one is already running.
- Current db (review #3): `db` = desired db (null = don't touch). Backend `USE` with backtick-escaped ident only when it differs; after each run reads `SELECT DATABASE()` and returns `currentDb` so the UI syncs after user-typed `USE`. If session conn drops, reconnect and log "session reset".
- Values (review #4/#5): column charset 63 (binary) → `0x…` hex (capped); else lossy UTF-8; each cell capped at 64KB with `…` marker. Dates/decimals via MySQL text form. `lastInsertId` as string. Grid keys columns by index (duplicate names ok).
- Multiple result sets (review #6): show first set only, drain the rest.
- Grid UX (review #7): click selects cell, Cmd+C copies cell, double-click opens full value; empty result still shows columns.
- Statement-under-cursor: split by `;` ignoring quotes/comments (small tokenizer). Multi-statement execution: not in MVP.
- Config file: `~/Library/Application Support/<bundle id>/connections.json` (no passwords).
- Tree autocomplete: when a db is selected, feed loaded table/column names to CodeMirror schema.

## Out of scope (later)
Inline cell editing, SSH tunnel, TLS options, multiple editor tabs, export CSV, ERD, other DB engines, read-only mode.

## Risks
- Large text/blob cells: truncate cell display to 1000 chars in grid.
- WebView memory with big results: capped by maxRows + virtualization.
- Keychain prompt on unsigned dev builds: acceptable for local use.

## Verification
- `node --test` for the TS statement splitter actually used by the editor; `cargo test` + an ignored-by-default integration test against local docker MySQL.
- `pnpm build` / `svelte-check` pass.
- Launch `pnpm tauri dev`, connect to 127.0.0.1:33307 (root/test), expand tree, run SELECT, cancel `SELECT SLEEP(30)`, resize panes; screenshot.
