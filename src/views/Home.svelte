<script lang="ts">
  import {
    dashboard,
    dropModule,
    moduleMeta,
    moveModule,
    resetDashboard,
    toggleModule,
    type DashboardModuleId,
  } from "../lib/state/dashboard.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import PanelSection from "../lib/components/PanelSection.svelte";
  import AccountModule from "../lib/components/dashboard/AccountModule.svelte";
  import ProjectsModule from "../lib/components/dashboard/ProjectsModule.svelte";
  import WatchtowerModule from "../lib/components/dashboard/WatchtowerModule.svelte";
  import EnvironmentModule from "../lib/components/dashboard/EnvironmentModule.svelte";
  import ActivityModule from "../lib/components/dashboard/ActivityModule.svelte";
  import ShortcutsModule from "../lib/components/dashboard/ShortcutsModule.svelte";

  let dragging = $state<DashboardModuleId | null>(null);
  const visible = $derived(dashboard.order.filter((id) => !dashboard.hidden.includes(id)));
  const hiddenCount = $derived(dashboard.hidden.length);
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Home</h1>
      <p class="view-sub">Your environment, at a glance. Every module here reads local state.</p>
    </div>
    <button class="btn" class:active={dashboard.editing} onclick={() => (dashboard.editing = !dashboard.editing)}>
      <Icon name="sliders" size={13} />
      {dashboard.editing ? "Done" : "Customize"}
    </button>
  </div>

  {#if dashboard.editing}
    <div class="panel editor">
      <div class="editor-head">
        <span class="mono-label">dashboard modules</span>
        <button class="btn ghost" onclick={resetDashboard}>Reset layout</button>
      </div>
      <ul>
        {#each dashboard.order as id (id)}
          {@const meta = moduleMeta(id)}
          <li>
            <label>
              <input type="checkbox" checked={!dashboard.hidden.includes(id)} onchange={() => toggleModule(id)} />
              <span class="module-name">{meta.title}</span>
              <span class="mono-label">{meta.hint}</span>
            </label>
            <span class="arrows">
              <button
                class="btn ghost icon"
                aria-label={`Move ${meta.title} up`}
                onclick={() => moveModule(id, -1)}
                disabled={dashboard.order.indexOf(id) === 0}
              >
                <Icon name="chevron-up" size={12} />
              </button>
              <button
                class="btn ghost icon"
                aria-label={`Move ${meta.title} down`}
                onclick={() => moveModule(id, 1)}
                disabled={dashboard.order.indexOf(id) === dashboard.order.length - 1}
              >
                <Icon name="chevron-down" size={12} />
              </button>
            </span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if visible.length === 0}
    <div class="panel empty">
      <p>Every dashboard module is hidden.</p>
      <button class="btn" onclick={() => (dashboard.editing = true)}>Show modules</button>
    </div>
  {:else}
    <div class="grid">
      {#each visible as id (id)}
        {@const meta = moduleMeta(id)}
        <div
          class="slot"
          role="group"
          aria-label={meta.title}
          class:wide={meta.span === 2}
          class:dragging={dragging === id}
          class:editing={dashboard.editing}
          draggable={dashboard.editing}
          ondragstart={() => (dragging = id)}
          ondragover={(event) => {
            if (dragging) event.preventDefault();
          }}
          ondrop={() => {
            if (dragging) dropModule(dragging, id);
            dragging = null;
          }}
          ondragend={() => (dragging = null)}
        >
          {#if dashboard.editing}
            <span class="handle" title="Drag to reorder"><Icon name="grip" size={13} /></span>
          {/if}

          {#if id === "account"}
            <AccountModule />
          {:else if id === "projects"}
            <ProjectsModule />
          {:else if id === "watchtower"}
            <WatchtowerModule />
          {:else if id === "environment"}
            <EnvironmentModule />
          {:else if id === "activity"}
            <ActivityModule />
          {:else if id === "shortcuts"}
            <ShortcutsModule />
          {:else}
            <PanelSection title={meta.title} hint={meta.hint}>
              <p class="mono">Module not registered.</p>
            </PanelSection>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if hiddenCount > 0 && !dashboard.editing}
    <p class="mono-label hidden-note">{hiddenCount} module{hiddenCount === 1 ? "" : "s"} hidden</p>
  {/if}
</section>

<style>
  .view-head .btn.active {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    color: var(--accent);
  }

  .editor {
    margin-bottom: var(--gap);
    padding: 4px 10px 8px;
  }

  .editor-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 28px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 2px;
  }

  .editor li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 28px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  }

  .editor li:last-child {
    border-bottom: 0;
  }

  .editor label {
    display: flex;
    align-items: center;
    gap: 9px;
    cursor: pointer;
    min-width: 0;
  }

  .editor input {
    accent-color: var(--accent);
    width: 12px;
    height: 12px;
  }

  .module-name {
    font-size: var(--font-small);
    min-width: 120px;
  }

  .arrows {
    display: inline-flex;
    gap: 1px;
  }

  .arrows .btn {
    width: 22px;
    height: 21px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--gap);
    align-items: stretch;
  }

  .slot {
    position: relative;
    display: flex;
    min-width: 0;
    min-height: 190px;
  }

  .slot > :global(*) {
    flex: 1;
    min-width: 0;
  }

  .wide {
    grid-column: span 2;
    min-height: 150px;
  }

  .slot.dragging {
    opacity: 0.45;
  }

  .slot.editing {
    outline: 1px dashed var(--border-strong);
    outline-offset: 2px;
  }

  .handle {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 2;
    color: var(--text-faint);
    cursor: grab;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 18px;
    color: var(--text-muted);
    font-size: var(--font-small);
  }

  .hidden-note {
    margin-top: 10px;
  }

  @media (max-width: 1080px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .wide {
      grid-column: span 1;
    }
  }
</style>
