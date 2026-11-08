<script lang="ts">
  import { onMount } from "svelte";
  import PanelSection from "../PanelSection.svelte";
  import Icon from "../Icon.svelte";
  import { navigate } from "../../state/ui.svelte";
  import { relativeTime } from "../../format";
  import { loadWatchtower, watchtower } from "../../state/watchtower.svelte";

  const lastCheck = $derived(
    watchtower.lastPollAt
      ? relativeTime(Math.floor(watchtower.lastPollAt / 1000))
      : "never",
  );

  onMount(() => {
    if (!watchtower.loaded) void loadWatchtower();
  });
</script>

<PanelSection title="Watchtower" hint="repository monitoring">
  {#snippet action()}
    <span class="badge" class:ok={watchtower.enabled} class:warn={!watchtower.enabled}>
      {watchtower.enabled ? "watching" : "paused"}
    </span>
  {/snippet}

  <div class="status">
    <span class="dot" class:on={watchtower.enabled}></span>
    <span class="label">
      {watchtower.enabled ? "Monitoring is on" : "Monitoring is off"}
    </span>
    <span class="mono-label">{watchtower.loading ? "loading…" : `checked ${lastCheck}`}</span>
  </div>

  <dl class="counts">
    <div><dt>Watched repositories</dt><dd class="mono">{watchtower.repos.length}</dd></div>
    <div><dt>Unread alerts</dt><dd class="mono" class:accent={watchtower.unread > 0}>{watchtower.unread}</dd></div>
    <div><dt>Alert categories</dt><dd class="mono">{watchtower.categories.length}</dd></div>
  </dl>

  {#if watchtower.error}
    <p class="note tone-danger">{watchtower.error}</p>
  {:else}
    <p class="note">
      {#if watchtower.repos.length === 0}
        Add a favourite from Watchtower or the repository workspace. The first check records a
        baseline, so GRAFT never replays old activity as new.
      {:else}
        History is stored locally. Background checks are conditional and only run while
        monitoring is enabled.
      {/if}
    </p>
  {/if}

  <button class="btn ghost open" onclick={() => navigate("watchtower")}>
    <Icon name="radar" size={13} />
    Open Watchtower
  </button>
</PanelSection>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 2px 0 9px;
    border-bottom: 1px solid var(--border);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .dot.on {
    background: var(--ok);
  }

  .label {
    font-size: var(--font-small);
    color: var(--text);
  }

  .counts {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin: 2px 0 8px;
  }

  .counts div {
    display: flex;
    justify-content: space-between;
    padding: 3px 0;
    font-size: var(--font-small);
  }

  dt {
    color: var(--text-faint);
  }

  dd {
    margin: 0;
    color: var(--text-muted);
    font-size: 10.5px;
  }

  dd.accent {
    color: var(--accent);
  }

  .note {
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .open {
    width: 100%;
    margin-top: 9px;
  }
</style>
