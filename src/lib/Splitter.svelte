<script lang="ts">
  // Drag handle. `size` is the size of the panel *before* the handle (or after, when `invert`).
  let {
    size = $bindable(),
    dir,
    min = 120,
    max = 2000,
    invert = false,
  }: { size: number; dir: "x" | "y"; min?: number; max?: number; invert?: boolean } = $props();

  let startPos = 0;
  let startSize = 0;
  let dragging = false;

  function down(e: PointerEvent) {
    dragging = true;
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {}
    startPos = dir === "x" ? e.clientX : e.clientY;
    startSize = size;
    document.body.style.cursor = dir === "x" ? "col-resize" : "row-resize";
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const d = (dir === "x" ? e.clientX : e.clientY) - startPos;
    size = Math.min(max, Math.max(min, startSize + (invert ? -d : d)));
  }
  function up() {
    dragging = false;
    document.body.style.cursor = "";
  }
</script>

<div
  class="splitter {dir}"
  role="separator"
  aria-orientation={dir === "x" ? "vertical" : "horizontal"}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onlostpointercapture={up}
></div>

<style>
  .splitter {
    background: var(--border);
    touch-action: none;
    z-index: 2;
  }
  .splitter:hover {
    background: var(--accent);
  }
  .x {
    cursor: col-resize;
    width: 4px;
  }
  .y {
    cursor: row-resize;
    height: 4px;
  }
</style>
