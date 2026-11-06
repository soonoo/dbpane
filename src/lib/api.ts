import { invoke } from "@tauri-apps/api/core";

export type ConnConfig = { id: string; name: string; host: string; port: number; user: string; database?: string | null };
export type TableInfo = { name: string; kind: "TABLE" | "VIEW" };
export type ColumnInfo = { name: string; type: string; nullable: boolean; key: string; default?: string | null; extra: string };
export type QueryResult = {
  columns: string[];
  rows: (string | null)[][];
  truncated: boolean;
  affected: number | null;
  lastInsertId: string | null;
  currentDb: string | null;
  elapsedMs: number;
  notice: string | null;
};

export const api = {
  listConnections: () => invoke<ConnConfig[]>("list_connections"),
  saveConnection: (cfg: ConnConfig, password?: string) => invoke<ConnConfig>("save_connection", { cfg, password }),
  deleteConnection: (id: string) => invoke<void>("delete_connection", { id }),
  testConnection: (cfg: ConnConfig, password?: string) => invoke<string>("test_connection", { cfg, password }),
  openConnection: (id: string) => invoke<void>("open_connection", { id }),
  closeConnection: (id: string) => invoke<void>("close_connection", { id }),
  listDatabases: (id: string) => invoke<string[]>("list_databases", { id }),
  listTables: (id: string, db: string) => invoke<TableInfo[]>("list_tables", { id, db }),
  listColumns: (id: string, db: string, table: string) => invoke<ColumnInfo[]>("list_columns", { id, db, table }),
  schemaColumns: (id: string, db: string) => invoke<Record<string, string[]>>("schema_columns", { id, db }),
  runQuery: (id: string, db: string | null, sql: string, maxRows: number) =>
    invoke<QueryResult>("run_query", { id, db, sql, maxRows }),
  cancelQuery: (id: string) => invoke<boolean>("cancel_query", { id }),
};

export const quoteIdent = (s: string) => "`" + s.replace(/`/g, "``") + "`";
