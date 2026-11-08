<script lang="ts">
  import { appearance, toggleSidebar } from "../state/appearance.svelte";
  import { navState } from "../state/nav.svelte";
  import { ui, navigate } from "../state/ui.svelte";
  import { navGroups } from "../nav";
  import Icon from "./Icon.svelte";

  const sections = $derived(navGroups(navState.order, navState.hidden, navState.pinned));
</script>

<aside class="sidebar" class:collapsed={appearance.sidebarCollapsed}>
  <div class="brand">
    <svg class="mark" viewBox="0 0 1024 1024" aria-hidden="true">
      <path d="M 250 300 H 470 V 372 H 322 V 652 H 470 V 724 H 250 Z" fill="var(--accent)" />
      <path d="M 554 300 H 774 V 724 H 554 V 652 H 702 V 372 H 554 Z" fill="var(--text-faint)" />
      <rect x="482" y="470" width="60" height="84" rx="6" fill="var(--text)" />
    </svg>
    {#if !appearance.sidebarCollapsed}
      <div class="brand-text">
        <span class="name">GRAFT</span>
        <span class="mono-label tagline">Build · Connect · Extract</span>
      </div>
    {/if}
  </div>

  <nav aria-label="Primary">
    {#each sections as section (section.name)}
      {#if !appearance.sidebarCollapsed}
        <div class="section-label mono-label">{section.name}</div>
      {/if}
      <div class="section-items">
        {#each section.items as item (item.id)}
          <button
            class="nav-item"
            class:active={ui.view === item.id}
            class:pinned={navState.pinned.includes(item.id)}
            title={appearance.sidebarCollapsed ? item.label : undefined}
            aria-current={ui.view === item.id ? "page" : undefined}
            onclick={() => navigate(item.id)}
          >
            <span class="icon"><Icon name={item.id} size={15} /></span>
            {#if !appearance.sidebarCollapsed}
              <span class="label">{item.label}</span>
              {#if item.status === "planned"}
                <span class="phase mono">{item.phase}</span>
              {/if}
            {/if}
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="foot">
    {#if !appearance.sidebarCollapsed}
      <span class="mono-label">Phase 1 · github</span>
    {/if}
    <button
      class="collapse btn ghost icon"
      onclick={toggleSidebar}
      title={appearance.sidebarCollapsed ? "Expand sidebar (Ctrl+B)" : "Collapse sidebar (Ctrl+B)"}
      aria-label="Toggle sidebar"
    >
      <Icon name={appearance.sidebarCollapsed ? "chevron-right" : "chevron-left"} size={14} />
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-w);
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-elev);
    border-right: 1px solid var(--border);
    transition: width var(--motion);
    overflow: hidden;
  }

  .sidebar.collapsed {
    width: var(--sidebar-w-collapsed);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    height: var(--topbar-h);
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .collapsed .brand {
    justify-content: center;
    padding: 0;
  }

  .mark {
    width: 19px;
    height: 19px;
    flex: none;
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .name {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.16em;
    line-height: 1.1;
  }

  .tagline {
    font-size: 8.5px;
    letter-spacing: 0.07em;
    white-space: nowrap;
  }

  nav {
    flex: 1;
    overflow-y: auto;
    padding: 8px 6px 12px;
  }

  .section-label {
    padding: 10px 8px 4px;
    font-size: 9.5px;
  }

  .section-items {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: var(--row-h);
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-muted);
    font-size: var(--font-small);
    text-align: left;
    cursor: pointer;
    transition:
      background var(--motion),
      color var(--motion);
  }

  .collapsed .nav-item {
    justify-content: center;
    padding: 0;
  }

  .nav-item:hover {
    background: var(--surface-alt);
    color: var(--text);
  }

  .nav-item.active {
    background: var(--accent-soft);
    color: var(--text);
  }

  .nav-item.active::before {
    content: "";
    position: absolute;
    left: -6px;
    top: 5px;
    bottom: 5px;
    width: 2px;
    border-radius: 0 2px 2px 0;
    background: var(--accent);
  }

  .collapsed .nav-item.active::before {
    left: -3px;
  }

  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 17px;
    flex: none;
    color: inherit;
  }

  .nav-item.active .icon {
    color: var(--accent);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .phase {
    font-size: 9px;
    color: var(--text-faint);
    border: 1px solid var(--border);
    border-radius: 2px;
    padding: 0 3px;
    line-height: 13px;
  }

  .pinned .label::after {
    content: "·";
    color: var(--accent);
    margin-left: 5px;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    height: 38px;
    padding: 0 8px;
    border-top: 1px solid var(--border);
    flex: none;
  }

  .foot .mono-label {
    font-size: 9px;
    padding-left: 4px;
  }

  .collapse {
    margin-left: auto;
  }
</style>
