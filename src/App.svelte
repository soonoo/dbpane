<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, quoteIdent, type ConnConfig, type QueryResult } from "./lib/api";
  import Splitter from "./lib/Splitter.svelte";
  import Tree from "./lib/Tree.svelte";
  import Editor from "./lib/Editor.svelte";
  import Grid from "./lib/Grid.svelte";
  import ConnDialog from "./lib/ConnDialog.svelte";

  const ls = {
    get: (k: string, d: string) => {
      try {
        return localStorage.getItem(k) ?? d;
      } catch {
        return d;
      }
    },
    set: (k: string, v: string) => {
      try {
        localStorage.setItem(k, v);
      } catch {}
    },
  };

  let sideW = $state(+ls.get("sideW", "280"));
  let bottomH = $state(+ls.get("bottomH", "300"));
  $effect(() => ls.set("sideW", String(sideW)));
  $effect(() => ls.set("bottomH", String(bottomH)));

  let conns = $state<ConnConfig[]>([]);
  let active = $state<{ id: string; db: string | null } | null>(null);
  let dialog = $state<{ conn: ConnConfig | null } | null>(null);
  let running = $state(false);
  let maxRows = $state(+ls.get("maxRows", "1000"));
  let result = $state.raw<QueryResult | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<"results" | "log">("results");
  type Log = { time: string; text: string; kind: "ok" | "err" | "info"; sql?: string };
  let logs = $state<Log[]>([]);
  let logEl = $state<HTMLDivElement>();

  let editor: Editor;
  let tree: Tree;
  const schemaCache = new Map<string, Record<string, string[]>>();

  const activeConn = $derived(conns.find((c) => c.id === active?.id));

  function log(kind: Log["kind"], text: string, sql?: string) {
    logs.push({ time: new Date().toLocaleTimeString(), text, kind, sql });
    if (logs.length > 1000) logs.splice(0, logs.length - 1000);
    tick().then(() => logEl?.scrollTo(0, logEl.scrollHeight));
  }

  onMount(async () => {
    conns = await api.listConnections();
    if (!conns.length) dialog = { conn: null };
  });

  // schema-aware autocomplete for the active db
  $effect(() => {
    const a = active;
    if (!a?.db) return;
    const k = `${a.id}/${a.db}`;
    const cached = schemaCache.get(k);
    if (cached) return editor.setSchema(cached);
    api
      .schemaColumns(a.id, a.db)
      .then((s) => {
        schemaCache.set(k, s);
        if (active?.id === a.id && active?.db === a.db) editor.setSchema(s);
      })
      .catch((e) => log("err", "autocomplete schema: " + e));
  });

  async function run(sqlOverride?: string) {
    if (running) return;
    if (!active) return log("err", "Select a connection or database in the sidebar first.");
    const sql = sqlOverride ?? editor.runnable();
    if (!sql?.trim()) return;
    ls.set("editor", editor.getText());
    maxRows = Math.min(10000, Math.max(1, Math.floor(maxRows) || 1000));
    ls.set("maxRows", String(maxRows));
    running = true;
    error = null;
    const { id, db } = active;
    try {
      const r = await api.runQuery(id, db, sql, maxRows);
      result = r;
      tab = "results";
      if (r.notice) log("info", r.notice);
      const what = r.columns.length
        ? `${r.rows.length} row(s)${r.truncated ? ` (truncated at ${maxRows})` : ""}`
        : `${r.affected ?? 0} row(s) affected${r.lastInsertId ? `, last insert id ${r.lastInsertId}` : ""}`;
      log("ok", `${what} in ${r.elapsedMs} ms`, sql);
      if (active?.id === id && r.currentDb !== active.db) active = { id, db: r.currentDb };
    } catch (e) {
      error = String(e);
      log("err", error, sql);
    }
    running = false;
  }

  async function cancel() {
    if (!active) return;
    try {
      if (await api.cancelQuery(active.id)) log("info", "Cancel requested");
    } catch (e) {
      log("err", "cancel: " + e);
    }
  }

  async function onsaved(c: ConnConfig) {
    dialog = null;
    await tree.reset(c.id);
    for (const k of schemaCache.keys()) if (k.startsWith(c.id + "/")) schemaCache.delete(k);
    conns = await api.listConnections();
    if (active?.id === c.id) active = null;
    log("info", `Saved connection "${c.name}"`);
  }

  async function ondelete(c: ConnConfig) {
    if (!confirm(`Delete connection "${c.name}"?`)) return;
    try {
      await tree.reset(c.id);
      await api.deleteConnection(c.id);
      conns = await api.listConnections();
      if (active?.id === c.id) active = null;
    } catch (e) {
      log("err", String(e));
    }
  }

  function ontable(id: string, db: string, table: string) {
    active = { id, db };
    const sql = `SELECT * FROM ${quoteIdent(table)} LIMIT 200;`;
    editor.insertAndSelect(sql);
    run(sql);
  }
</script>

<div class="app" style:grid-template-columns="{sideW}px 4px 1fr">
  <aside>
    <div class="side-head">
      <span>Connections</span>
      <button title="New connection" onclick={() => (dialog = { conn: null })}>＋</button>
    </div>
    <div class="side-body">
      <Tree
        bind:this={tree}
        {conns}
        {active}
        onselect={(id, db) => (active = { id, db })}
        {ontable}
        onedit={(c) => (dialog = { conn: c })}
        {ondelete}
        onerror={(m) => log("err", m)}
      />
    </div>
  </aside>
  <Splitter dir="x" bind:size={sideW} min={160} max={700} />
  <main style:grid-template-rows="36px 1fr 4px {bottomH}px">
    <div class="toolbar">
      <button class="primary" onclick={() => run()} disabled={running || !active} title="Run (⌘↵)">▶ Run</button>
      <button onclick={cancel} disabled={!running}>■ Cancel</button>
      <span class="target">
        {#if activeConn}<b>{activeConn.name}</b> / {active?.db ?? "(no database)"}{:else}No connection selected{/if}
      </span>
      <span style="flex:1"></span>
      {#if running}<span class="spinner">running…</span>{/if}
      <label class="max">max rows <input type="number" bind:value={maxRows} min="1" max="10000" /></label>
    </div>
    <div class="editor-wrap">
      <Editor
        bind:this={editor}
        onrun={() => run()}
        initial={ls.get("editor", "-- ⌘↵ runs the selection, or the statement under the cursor\nSELECT 1;\n")}
      />
    </div>
    <Splitter dir="y" bind:size={bottomH} min={80} max={1200} invert />
    <section class="bottom">
      <div class="tabs">
        <button class:on={tab === "results"} onclick={() => (tab = "results")}>Results</button>
        <button class:on={tab === "log"} onclick={() => (tab = "log")}>Log ({logs.length})</button>
        {#if tab === "log"}<button class="clear" onclick={() => (logs = [])}>Clear</button>{/if}
      </div>
      <div class="tab-body">
        {#if tab === "results"}
          {#if error}
            <pre class="error">{error}</pre>
          {:else if result?.columns.length}
            <Grid {result} />
          {:else if result}
            <div class="msg">{result.affected ?? 0} row(s) affected · {result.elapsedMs} ms</div>
          {:else}
            <div class="msg">Run a query to see results.</div>
          {/if}
        {:else}
          <div class="log" bind:this={logEl}>
            {#each logs as l, i (i)}
              <div class="log-row {l.kind}">
                <span class="time">{l.time}</span>
                <span>{l.text}</span>
                {#if l.sql}<code>{l.sql.length > 500 ? l.sql.slice(0, 500) + "…" : l.sql}</code>{/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>
  </main>
</div>

{#if dialog}
  <ConnDialog conn={dialog.conn} onclose={() => (dialog = null)} {onsaved} />
{/if}

<style>
  .app {
    display: grid;
    height: 100vh;
  }
  aside,
  main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  aside {
    display: flex;
    flex-direction: column;
    background: var(--panel);
  }
  .side-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 36px;
    padding: 0 8px;
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .side-body {
    flex: 1;
    overflow: auto;
  }
  main {
    display: grid;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    font-size: 12px;
  }
  .target {
    color: var(--muted);
    margin-left: 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .target b {
    color: var(--fg);
  }
  .spinner {
    color: var(--accent);
  }
  .max {
    color: var(--muted);
    white-space: nowrap;
  }
  .max input {
    width: 70px;
  }
  .editor-wrap {
    min-height: 0;
    overflow: hidden;
  }
  .bottom {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg);
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 6px;
    height: 28px;
    align-items: end;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    border: none;
    border-radius: 4px 4px 0 0;
    background: none;
    color: var(--muted);
    padding: 4px 12px;
  }
  .tabs button.on {
    background: var(--bg);
    color: var(--fg);
  }
  .tabs .clear {
    margin-left: auto;
  }
  .tab-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .msg {
    padding: 12px;
    color: var(--muted);
  }
  .error {
    margin: 0;
    padding: 12px;
    color: var(--err);
    white-space: pre-wrap;
    font-family: var(--mono);
    font-size: 12px;
  }
  .log {
    height: 100%;
    overflow: auto;
    font-family: var(--mono);
    font-size: 12px;
    padding: 4px 0;
  }
  .log-row {
    display: flex;
    gap: 10px;
    padding: 2px 10px;
    align-items: baseline;
  }
  .log-row .time {
    color: var(--muted);
    flex: none;
  }
  .log-row.err span:nth-child(2) {
    color: var(--err);
  }
  .log-row.ok span:nth-child(2) {
    color: var(--ok);
  }
  .log-row code {
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
