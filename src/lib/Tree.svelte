<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { api, type ColumnInfo, type ConnConfig, type TableInfo } from "./api";

  let {
    conns,
    active,
    onselect,
    ontable,
    onedit,
    ondelete,
    onerror,
  }: {
    conns: ConnConfig[];
    active: { id: string; db: string | null } | null;
    onselect: (id: string, db: string | null) => void;
    ontable: (id: string, db: string, table: string) => void;
    onedit: (c: ConnConfig) => void;
    ondelete: (c: ConnConfig) => void;
    onerror: (msg: string) => void;
  } = $props();

  const open = new SvelteSet<string>();
  const loading = new SvelteSet<string>();
  let dbs = $state<Record<string, string[]>>({});
  let tables = $state<Record<string, TableInfo[]>>({});
  let columns = $state<Record<string, ColumnInfo[]>>({});

  async function toggle(key: string, load: () => Promise<void>) {
    if (open.has(key)) return open.delete(key);
    loading.add(key);
    try {
      await load();
      open.add(key);
    } catch (e) {
      onerror(String(e));
    }
    loading.delete(key);
  }

  const toggleConn = (c: ConnConfig) =>
    toggle(c.id, async () => {
      await api.openConnection(c.id);
      dbs[c.id] ??= await api.listDatabases(c.id);
      onselect(c.id, c.database || null);
    });
  const toggleDb = (id: string, db: string) => {
    onselect(id, db);
    return toggle(`${id}/${db}`, async () => {
      tables[`${id}/${db}`] ??= await api.listTables(id, db);
    });
  };
  const toggleTable = (id: string, db: string, t: string) =>
    toggle(`${id}/${db}/${t}`, async () => {
      columns[`${id}/${db}/${t}`] ??= await api.listColumns(id, db, t);
    });

  /** Drop cached metadata and close the connection. */
  export async function reset(id: string) {
    for (const k of [...open]) if (k === id || k.startsWith(id + "/")) open.delete(k);
    const drop = (o: Record<string, unknown>) => {
      for (const k of Object.keys(o)) if (k === id || k.startsWith(id + "/")) delete o[k];
    };
    drop(dbs);
    drop(tables);
    drop(columns);
    await api.closeConnection(id).catch(() => {});
  }

  async function refresh(c: ConnConfig) {
    await reset(c.id);
    await toggleConn(c);
  }
</script>

{#snippet row(key: string, icon: string, label: string, extra = "", isOpen: boolean | null = null)}
  <span class="twisty">{isOpen === null ? "" : loading.has(key) ? "…" : isOpen ? "▾" : "▸"}</span>
  <span class="icon">{icon}</span>
  <span class="label">{label}</span>
  {#if extra}<span class="extra">{extra}</span>{/if}
{/snippet}

<div class="tree">
  {#each conns as c (c.id)}
    <div
      class="node conn"
      class:active={active?.id === c.id}
      role="treeitem"
      aria-selected={active?.id === c.id}
      aria-expanded={open.has(c.id)}
      tabindex="0"
      onclick={() => toggleConn(c)}
      onkeydown={(e) => e.key === "Enter" && toggleConn(c)}
    >
      {@render row(c.id, "⛁", c.name, `${c.user}@${c.host}:${c.port}`, open.has(c.id))}
      <span class="tools">
        <button title="Refresh" onclick={(e) => (e.stopPropagation(), refresh(c))}>↻</button>
        <button title="Edit" onclick={(e) => (e.stopPropagation(), onedit(c))}>✎</button>
        <button title="Delete" onclick={(e) => (e.stopPropagation(), ondelete(c))}>✕</button>
      </span>
    </div>
    {#if open.has(c.id)}
      {#each dbs[c.id] ?? [] as db (db)}
        {@const dk = `${c.id}/${db}`}
        <div
          class="node d1"
          class:active={active?.id === c.id && active?.db === db}
          role="treeitem"
          aria-selected={active?.id === c.id && active?.db === db}
          aria-expanded={open.has(dk)}
          tabindex="0"
          onclick={() => toggleDb(c.id, db)}
          onkeydown={(e) => e.key === "Enter" && toggleDb(c.id, db)}
        >
          {@render row(dk, "◫", db, "", open.has(dk))}
        </div>
        {#if open.has(dk)}
          {#each tables[dk] ?? [] as t (t.name)}
            {@const tk = `${dk}/${t.name}`}
            <div
              class="node d2"
              role="treeitem"
              aria-selected="false"
              aria-expanded={open.has(tk)}
              tabindex="0"
              title="Double-click: SELECT * … LIMIT 200"
              onclick={() => toggleTable(c.id, db, t.name)}
              ondblclick={() => ontable(c.id, db, t.name)}
              onkeydown={(e) => e.key === "Enter" && toggleTable(c.id, db, t.name)}
            >
              {@render row(tk, t.kind === "VIEW" ? "◇" : "▦", t.name, "", open.has(tk))}
            </div>
            {#if open.has(tk)}
              {#each columns[tk] ?? [] as col (col.name)}
                <div class="node d3" role="treeitem" aria-selected="false" title={col.extra || col.type}>
                  {@render row(
                    
                    "",
                    col.key === "PRI" ? "🔑" : col.key ? "◆" : "·",
                    col.name,
                    col.type + (col.nullable ? "" : " not null"),
                  )}
                </div>
              {/each}
            {/if}
          {:else}
            <div class="node d2 muted">(no tables)</div>
          {/each}
        {/if}
      {/each}
    {/if}
  {/each}
</div>

<style>
  .tree {
    font-size: 12.5px;
    user-select: none;
    padding: 4px 0;
  }
  .node {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding-right: 6px;
    white-space: nowrap;
    cursor: default;
    outline: none;
  }
  .node:hover,
  .node:focus-visible {
    background: var(--hover);
  }
  .node.active {
    background: var(--selected);
  }
  .conn {
    padding-left: 4px;
  }
  .d1 {
    padding-left: 18px;
  }
  .d2 {
    padding-left: 32px;
  }
  .d3 {
    padding-left: 46px;
  }
  .twisty {
    width: 12px;
    color: var(--muted);
    flex: none;
  }
  .icon {
    width: 14px;
    text-align: center;
    color: var(--accent);
    flex: none;
    font-size: 11px;
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .extra,
  .muted {
    color: var(--muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tools {
    margin-left: auto;
    display: none;
    gap: 2px;
  }
  .conn:hover .tools {
    display: flex;
  }
  .tools button {
    padding: 0 4px;
    background: none;
    border: none;
    color: var(--muted);
  }
  .tools button:hover {
    color: var(--fg);
  }
</style>
