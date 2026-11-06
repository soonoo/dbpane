<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView, basicSetup } from "codemirror";
  import { keymap } from "@codemirror/view";
  import { Compartment, Prec } from "@codemirror/state";
  import { sql, MySQL } from "@codemirror/lang-sql";
  import { oneDark } from "@codemirror/theme-one-dark";
  import { statementAt } from "./split";

  let { onrun, initial = "" }: { onrun: () => void; initial?: string } = $props();

  let host: HTMLDivElement;
  let view: EditorView;
  const lang = new Compartment();

  onMount(() => {
    view = new EditorView({
      parent: host,
      doc: initial,
      extensions: [
        Prec.highest(keymap.of([{ key: "Mod-Enter", run: () => (onrun(), true) }])),
        basicSetup,
        lang.of(sql({ dialect: MySQL, upperCaseKeywords: true })),
        oneDark,
        EditorView.theme({ "&": { height: "100%" }, ".cm-scroller": { fontFamily: "var(--mono)" } }),
      ],
    });
    return () => view.destroy();
  });

  export function setSchema(schema: Record<string, string[]>, defaultTable?: string) {
    view.dispatch({
      effects: lang.reconfigure(sql({ dialect: MySQL, upperCaseKeywords: true, schema, defaultTable })),
    });
  }

  export function getText() {
    return view.state.doc.toString();
  }

  /** Selection if any, else the statement under the cursor (highlighted as selection). */
  export function runnable(): string | null {
    const sel = view.state.selection.main;
    if (!sel.empty) return view.state.sliceDoc(sel.from, sel.to);
    const s = statementAt(view.state.doc.toString(), sel.head);
    return s?.sql ?? null;
  }

  export function insertAndSelect(text: string) {
    const end = view.state.doc.length;
    const prefix = end === 0 ? "" : view.state.doc.sliceString(end - 1) === "\n" ? "\n" : "\n\n";
    const from = end + prefix.length;
    view.dispatch({ changes: { from: end, insert: prefix + text }, selection: { anchor: from + text.length } });
    view.focus();
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
    overflow: hidden;
  }
</style>
