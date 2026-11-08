<script lang="ts">
  import { session } from "../state/session.svelte";
  import { currentTheme } from "../state/appearance.svelte";
  import { ui } from "../state/ui.svelte";

  const theme = $derived(currentTheme());
</script>

<footer>
  <div class="left">
    <span class="item mono">
      <span class="dot" class:on={session.online}></span>
      sqlite · schema {session.info?.schemaVersion ?? "—"}
    </span>
    <span class="sep"></span>
    <span class="item mono">theme {theme.id}</span>
    <span class="sep"></span>
    <span class="item mono">layout local</span>
  </div>
  <div class="right">
    {#if ui.toast}
      <span class="toast" class:danger={ui.toast.tone === "danger"} class:ok={ui.toast.tone === "ok"}>
        {ui.toast.text}
      </span>
    {/if}
    <span class="item mono">v{session.info?.version ?? "0.1.0"}</span>
  </div>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    height: var(--statusbar-h);
    padding: 0 12px;
    border-top: 1px solid var(--border);
    background: var(--bg-elev);
    flex: none;
  }

  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .item {
    font-size: 9.5px;
    letter-spacing: 0.04em;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .sep {
    width: 1px;
    height: 9px;
    background: var(--border);
  }

  .dot {
    display: inline-block;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    margin-right: 4px;
    background: var(--danger);
    vertical-align: middle;
  }

  .dot.on {
    background: var(--ok);
  }

  .toast {
    font-size: 10.5px;
    color: var(--text-muted);
    padding: 0 6px;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    max-width: 420px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toast.ok {
    color: var(--ok);
  }

  .toast.danger {
    color: var(--danger);
  }
</style>
