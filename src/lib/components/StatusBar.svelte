<script lang="ts">
  import { session } from "../state/session.svelte";
  import { currentTheme } from "../state/appearance.svelte";
  import { ui } from "../state/ui.svelte";
  import { github, loadRateLimit } from "../state/github.svelte";

  const theme = $derived(currentTheme());
  const core = $derived(github.rate?.core ?? null);
  const quotaTone = $derived(
    core ? (core.remaining === 0 ? "danger" : core.remaining / Math.max(core.limit, 1) < 0.1 ? "warn" : "") : "",
  );
  const quotaTitle = $derived.by(() => {
    if (!core) return "GitHub API quota";
    const rate = github.rate;
    const reset = Math.max(0, Math.ceil((core.reset * 1000 - Date.now()) / 60000));
    const parts = [`core ${core.remaining.toLocaleString()}/${core.limit.toLocaleString()}`];
    if (rate) parts.push(`search ${rate.search.remaining.toLocaleString()}/${rate.search.limit.toLocaleString()}`);
    if (rate?.graphql) parts.push(`graphql ${rate.graphql.remaining.toLocaleString()}/${rate.graphql.limit.toLocaleString()}`);
    parts.push(reset === 0 ? "resets now" : `resets in ${reset}m`);
    parts.push("click to refresh");
    return parts.join(" · ");
  });
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
    {#if core}
      <span class="sep"></span>
      <button
        class="item mono quota"
        class:warn={quotaTone === "warn"}
        class:danger={quotaTone === "danger"}
        title={quotaTitle}
        onclick={() => loadRateLimit(true)}
      >
        api {core.remaining.toLocaleString()}/{core.limit.toLocaleString()}
      </button>
    {/if}
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

  .quota {
    padding: 0;
    border: 0;
    background: none;
    font-family: inherit;
    cursor: pointer;
  }

  .quota:hover {
    color: var(--text-muted);
  }

  .quota.warn {
    color: var(--warn);
  }

  .quota.danger {
    color: var(--danger);
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
