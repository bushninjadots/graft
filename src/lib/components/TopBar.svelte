<script lang="ts">
  import type { NavItem } from "../nav";
  import { openPalette } from "../state/ui.svelte";
  import { session } from "../state/session.svelte";
  import Icon from "./Icon.svelte";

  let { item }: { item: NavItem } = $props();
</script>

<header>
  <div class="crumbs">
    <span class="root mono">GRAFT</span>
    <span class="slash" aria-hidden="true">/</span>
    <span class="current">{item.label}</span>
    {#if item.status === "planned"}
      <span class="badge warn">Phase {item.phase}</span>
    {:else if item.id !== "settings"}
      <span class="badge accent">Live</span>
    {/if}
  </div>

  <div class="tools">
    <span class="engine mono-label" title={session.online ? "Rust engine connected" : "Engine unreachable"}>
      <span class="dot" class:on={session.online}></span>
      {session.online ? "engine online" : "engine offline"}
    </span>

    <button class="cmd" onclick={openPalette} aria-label="Open command palette">
      <Icon name="search" size={13} />
      <span>Search or run a command</span>
      <span class="kbd">Ctrl K</span>
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    height: var(--topbar-h);
    padding: 0 12px 0 14px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .root {
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--text-faint);
  }

  .slash {
    color: var(--border-strong);
  }

  .current {
    font-size: var(--font-small);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
  }

  .engine {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9.5px;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--danger);
  }

  .dot.on {
    background: var(--ok);
  }

  .cmd {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 7px 0 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-faint);
    font-size: 11.5px;
    cursor: pointer;
    transition:
      border-color var(--motion),
      color var(--motion),
      background var(--motion);
  }

  .cmd:hover {
    color: var(--text-muted);
    border-color: var(--border-strong);
    background: var(--surface-alt);
  }

  .cmd .kbd {
    margin-left: 4px;
  }

  @media (max-width: 1080px) {
    .cmd span:not(.kbd) {
      display: none;
    }
    .engine {
      display: none;
    }
  }
</style>
