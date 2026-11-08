<script lang="ts">
  import { session, refreshTools } from "../../state/session.svelte";
  import { navigate } from "../../state/ui.svelte";
  import PanelSection from "../PanelSection.svelte";
  import Icon from "../Icon.svelte";

  const found = $derived(session.tools.filter((tool) => tool.found));
  const missing = $derived(session.tools.length - found.length);
  const shown = $derived(found.slice(0, 5));

  const summary = $derived.by(() => {
    if (!session.online) return "backend offline";
    if (session.toolsLoading) return "scanning PATH…";
    if (session.tools.length === 0) return "not scanned";
    return `${found.length} found · ${missing} missing`;
  });
</script>

<PanelSection title="Environment" hint={summary}>
  {#snippet action()}
    <button
      class="btn ghost icon"
      title="Re-scan installed tools"
      aria-label="Re-scan installed tools"
      onclick={() => refreshTools()}
      disabled={!session.online || session.toolsLoading}
    >
      <span class:spin={session.toolsLoading}><Icon name="refresh" size={13} /></span>
    </button>
  {/snippet}

  {#if !session.online}
    <p class="state">The Rust engine is not reachable, so PATH cannot be inspected.</p>
  {:else if session.tools.length === 0 && !session.toolsLoading}
    <p class="state">No scan has run yet.</p>
  {:else}
    <ul class="tools">
      {#each shown as tool (tool.id)}
        <li title={tool.path ?? tool.name}>
          <span class="tick ok" aria-hidden="true"></span>
          <span class="name">{tool.name}</span>
          <span class="version mono">{tool.version ?? "installed"}</span>
        </li>
      {/each}
    </ul>
    <div class="foot">
      <span class="mono-label">{missing} tool{missing === 1 ? "" : "s"} not found</span>
      <button class="btn ghost more" onclick={() => navigate("settings")}>Open settings</button>
    </div>
  {/if}
</PanelSection>

<style>
  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
    line-height: 1.5;
  }

  .tools li {
    display: grid;
    grid-template-columns: 10px 92px 1fr;
    align-items: center;
    gap: 6px;
    height: calc(var(--row-h) - 2px);
  }

  .tick {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .tick.ok {
    background: var(--ok);
  }

  .name {
    font-size: var(--font-small);
    color: var(--text);
  }

  .version {
    font-size: 9.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }

  .more {
    height: 20px;
    padding: 0 5px;
    font-size: 10px;
  }

  .spin {
    display: inline-flex;
    animation: spin 900ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
