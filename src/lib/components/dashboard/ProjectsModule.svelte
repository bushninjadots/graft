<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Project } from "../../api";
  import { relativeTime } from "../../format";
  import { navigate } from "../../state/ui.svelte";
  import PanelSection from "../PanelSection.svelte";
  import Icon from "../Icon.svelte";

  let projects = $state<Project[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      projects = await api.listProjects();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  });

  const shown = $derived(projects.slice(0, 4));
</script>

<PanelSection title="My Projects" hint="local workspaces">
  {#snippet action()}
    <button class="btn ghost icon" title="Open My Projects" onclick={() => navigate("projects")}>
      <Icon name="arrow-up-right" size={13} />
    </button>
  {/snippet}

  {#if loading}
    <p class="state mono">reading database…</p>
  {:else if error}
    <p class="state tone-danger">Backend unavailable.</p>
    <p class="note">{error}</p>
  {:else if projects.length === 0}
    <div class="empty">
      <p class="state">No projects yet.</p>
      <p class="note">A project links a local directory to one or more repositories.</p>
      <button class="btn primary" onclick={() => navigate("projects")}>
        <Icon name="plus" size={13} /> Create a project
      </button>
    </div>
  {:else}
    <ul class="list">
      {#each shown as project (project.id)}
        <li>
          <Icon name="folder-git" size={14} />
          <span class="name">{project.name}</span>
          <span class="meta mono" title={project.path || "no directory linked"}>
            {project.path ? project.path.split("/").slice(-1)[0] : "no path"}
          </span>
          <span class="time mono" title={new Date(project.createdAt * 1000).toLocaleString()}>
            {relativeTime(project.createdAt)}
          </span>
        </li>
      {/each}
    </ul>
    {#if projects.length > shown.length}
      <button class="btn ghost more" onclick={() => navigate("projects")}>
        {projects.length - shown.length} more…
      </button>
    {/if}
  {/if}
</PanelSection>

<style>
  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .note {
    margin-top: 4px;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .empty .btn {
    margin-top: 4px;
  }

  .list li {
    display: grid;
    grid-template-columns: 16px 1fr auto auto;
    align-items: center;
    gap: 7px;
    height: var(--row-h);
    color: var(--text-muted);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .list li:last-child {
    border-bottom: 0;
  }

  .name {
    color: var(--text);
    font-size: var(--font-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta,
  .time {
    font-size: 9.5px;
    color: var(--text-faint);
    max-width: 110px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .more {
    margin-top: 4px;
    height: 22px;
    padding: 0 6px;
    font-size: 10.5px;
  }
</style>
