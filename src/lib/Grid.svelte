<script lang="ts">
  import type { QueryResult } from "./api";

  let { result }: { result: QueryResult } = $props();

  const ROW_H = 24;
  const COL_W = 180;
  const IDX_W = 56;
  const OVERSCAN = 10;

  let scroller: HTMLDivElement;
  let scrollTop = $state(0);
  let viewH = $state(400);
  let sel = $state<{ r: number; c: number } | null>(null);
  let detail = $state<string | null>(null);

  $effect(() => {
    result; // reset on new result
    sel = null;
    if (scroller) scroller.scrollTop = 0;
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  const last = $derived(Math.min(result.rows.length, Math.ceil((scrollTop + viewH) / ROW_H) + OVERSCAN));
  const cols = $derived(`${IDX_W}px repeat(${result.columns.length}, ${COL_W}px)`);
  const width = $derived(IDX_W + result.columns.length * COL_W);

  function key(e: KeyboardEvent) {
    if (!sel) return;
    if ((e.metaKey || e.ctrlKey) && e.key === "c") {
      navigator.clipboard.writeText(result.rows[sel.r][sel.c] ?? "NULL");
      e.preventDefault();
    } else if (e.key === "Enter") {
      detail = result.rows[sel.r][sel.c] ?? "NULL";
    } else {
      const d = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] }[e.key];
      if (!d) return;
      e.preventDefault();
      const r = Math.min(result.rows.length - 1, Math.max(0, sel.r + d[0]));
      const c = Math.min(result.columns.length - 1, Math.max(0, sel.c + d[1]));
      sel = { r, c };
      if (r * ROW_H < scroller.scrollTop) scroller.scrollTop = r * ROW_H;
      else if ((r + 2) * ROW_H > scroller.scrollTop + viewH) scroller.scrollTop = (r + 2) * ROW_H - viewH;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="grid"
  bind:this={scroller}
  bind:clientHeight={viewH}
  onscroll={() => (scrollTop = scroller.scrollTop)}
  onkeydown={key}
  tabindex="0"
  role="grid"
>
  <div class="row head" style:grid-template-columns={cols} style:width="{width}px">
    <div class="cell idx">#</div>
    {#each result.columns as c, i (i)}<div class="cell" title={c}>{c}</div>{/each}
  </div>
  <div style:height="{result.rows.length * ROW_H}px" style:width="{width}px" style:position="relative">
    {#each result.rows.slice(first, last) as row, k (first + k)}
      {@const r = first + k}
      <div class="row" style:grid-template-columns={cols} style:transform="translateY({r * ROW_H}px)">
        <div class="cell idx">{r + 1}</div>
        {#each row as v, c (c)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="cell"
            class:null={v === null}
            class:sel={sel?.r === r && sel?.c === c}
            role="gridcell"
            tabindex="-1"
            onclick={() => (sel = { r, c })}
            ondblclick={() => (detail = v ?? "NULL")}
          >
            {v === null ? "NULL" : v.length > 300 ? v.slice(0, 300) + "…" : v}
          </div>
        {/each}
      </div>
    {/each}
  </div>
  {#if result.rows.length === 0}<div class="empty">(no rows)</div>{/if}
</div>

{#if detail !== null}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="modal-bg" onclick={() => (detail = null)}>
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <textarea readonly value={detail}></textarea>
      <div class="actions">
        <button onclick={() => navigator.clipboard.writeText(detail ?? "")}>Copy</button>
        <button onclick={() => (detail = null)}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .grid {
    height: 100%;
    overflow: auto;
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
    position: relative;
  }
  .row {
    display: grid;
    height: 24px;
    position: absolute;
    left: 0;
    top: 0;
  }
  .head {
    position: sticky;
    z-index: 1;
    background: var(--panel2);
    font-weight: 600;
  }
  .cell {
    padding: 0 8px;
    line-height: 24px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    cursor: default;
  }
  .idx {
    color: var(--muted);
    text-align: right;
    background: var(--panel2);
  }
  .null {
    color: var(--muted);
    font-style: italic;
  }
  .sel {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .empty {
    padding: 12px;
    color: var(--muted);
  }
  textarea {
    width: 70vw;
    height: 60vh;
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
