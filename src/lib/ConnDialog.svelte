<script lang="ts">
  import { untrack } from "svelte";
  import { api, type ConnConfig } from "./api";

  let { conn, onclose, onsaved }: { conn: ConnConfig | null; onclose: () => void; onsaved: (c: ConnConfig) => void } =
    $props();

  const init = untrack(() => conn); // dialog is recreated per open
  const editing = !!init?.id;
  let cfg = $state<ConnConfig>(
    init ? { ...init } : { id: "", name: "local", host: "127.0.0.1", port: 3306, user: "root", database: "" },
  );
  let password = $state("");
  let pwTouched = $state(!editing);
  let status = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  // undefined = keep stored password (only possible when editing and untouched)
  const pw = () => (pwTouched ? password : undefined);

  async function test() {
    busy = true;
    status = null;
    try {
      status = { ok: true, text: "Connected: MySQL " + (await api.testConnection($state.snapshot(cfg), pw())) };
    } catch (e) {
      status = { ok: false, text: String(e) };
    }
    busy = false;
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    try {
      onsaved(await api.saveConnection($state.snapshot(cfg), pw()));
    } catch (err) {
      status = { ok: false, text: String(err) };
    }
    busy = false;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<div class="modal-bg" onclick={onclose}>
  <form class="modal form" onclick={(e) => e.stopPropagation()} onsubmit={save}>
    <h3>{editing ? "Edit connection" : "New connection"}</h3>
    <label>Name <input bind:value={cfg.name} required /></label>
    <div class="two">
      <label>Host <input bind:value={cfg.host} required /></label>
      <label class="port">Port <input type="number" bind:value={cfg.port} min="1" max="65535" required /></label>
    </div>
    <label>User <input bind:value={cfg.user} required autocomplete="off" /></label>
    <label
      >Password
      <input
        type="password"
        bind:value={password}
        oninput={() => (pwTouched = true)}
        placeholder={editing && !pwTouched ? "(unchanged, stored in Keychain)" : ""}
        autocomplete="off"
      />
    </label>
    <label>Default database <input bind:value={cfg.database} placeholder="(optional)" /></label>
    {#if status}<p class:err={!status.ok} class="status">{status.text}</p>{/if}
    <div class="actions">
      <button type="button" onclick={test} disabled={busy}>Test</button>
      <span style="flex:1"></span>
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={busy}>Save</button>
    </div>
  </form>
</div>

<style>
  .form {
    width: 420px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0 0 4px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .two {
    display: flex;
    gap: 8px;
  }
  .two label:first-child {
    flex: 1;
  }
  .port {
    width: 90px;
  }
  .status {
    margin: 0;
    font-size: 12px;
    color: var(--ok);
    word-break: break-word;
  }
  .err {
    color: var(--err);
  }
</style>
