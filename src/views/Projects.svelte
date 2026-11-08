<script lang="ts">
  import { onMount } from "svelte";
  import { api, type GitStatus, type Project } from "../lib/api";
  import { absoluteTime, plural, relativeTime } from "../lib/format";
  import { session } from "../lib/state/session.svelte";
  import { notify } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  let projects = $state<Project[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let creating = $state(false);
  let submitting = $state(false);
  let formError = $state<string | null>(null);
  let form = $state({ name: "", path: "", description: "" });

  let confirmId = $state<number | null>(null);
  let gitStates = $state<Record<number, GitStatus | "loading">>({});
  let detecting = $state(false);
  let detected = $state<GitStatus | null>(null);

  async function load(): Promise<void> {
    loading = true;
    loadError = null;
    try {
      projects = await api.listProjects();
      void refreshGit();
    } catch (cause) {
      loadError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function resetForm(): void {
    form = { name: "", path: "", description: "" };
    formError = null;
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    formError = null;
    if (!form.name.trim()) {
      formError = "A project name is required.";
      return;
    }
    submitting = true;
    try {
      const project = await api.createProject(form.name, form.path, form.description);
      projects = [project, ...projects];
      void refreshGit();
      creating = false;
      resetForm();
      notify(`Project “${project.name}” created`, "ok");
    } catch (cause) {
      formError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      submitting = false;
    }
  }

  /** Reads branch, change count and remote for every project that links a directory. */
  async function refreshGit(): Promise<void> {
    const targets = projects.filter((project) => project.path.trim());
    if (targets.length === 0) {
      gitStates = {};
      return;
    }

    const pending: Record<number, GitStatus | "loading"> = { ...gitStates };
    for (const project of targets) pending[project.id] = "loading";
    gitStates = pending;

    await Promise.all(
      targets.map(async (project) => {
        const status = await api.gitStatus(project.path).catch<GitStatus>((cause) => ({
          path: project.path,
          isRepo: false,
          toplevel: null,
          branch: null,
          detached: false,
          ahead: 0,
          behind: 0,
          changed: 0,
          remote: null,
          github: null,
          error: cause instanceof Error ? cause.message : String(cause),
        }));
        gitStates = { ...gitStates, [project.id]: status };
      }),
    );
  }

  function gitInfo(id: number): GitStatus | null {
    const value = gitStates[id];
    return value && value !== "loading" ? value : null;
  }

  async function detectDirectory(): Promise<void> {
    const path = form.path.trim();
    if (!path) {
      detected = null;
      return;
    }
    detecting = true;
    try {
      detected = await api.gitStatus(path);
      if (!form.name.trim() && detected.toplevel) {
        form.name = detected.toplevel.split("/").filter(Boolean).pop() ?? "";
      }
    } finally {
      detecting = false;
    }
  }

  async function remove(id: number): Promise<void> {
    try {
      await api.deleteProject(id);
      projects = projects.filter((project) => project.id !== id);
      confirmId = null;
      notify("Project removed", "ok");
    } catch (cause) {
      notify(cause instanceof Error ? cause.message : String(cause), "danger");
    }
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">My Projects</h1>
      <p class="view-sub">
        Projects group repositories, branches and local directories. Only the directories you link
        are inspected — GRAFT never scans the filesystem on its own.
      </p>
    </div>
    <button
      class="btn primary"
      onclick={() => {
        creating = !creating;
        if (!creating) resetForm();
      }}
    >
      <Icon name={creating ? "x" : "plus"} size={13} />
      {creating ? "Cancel" : "New project"}
    </button>
  </div>

  {#if creating}
    <form class="panel form" onsubmit={submit}>
      <div class="form-head mono-label">new local project</div>
      <div class="fields">
        <label>
          <span class="mono-label">Name</span>
          <input class="field" bind:value={form.name} placeholder="MOOI" maxlength="120" required />
        </label>
        <label>
          <span class="mono-label">Directory <span class="faint">(optional)</span></span>
          <span class="path-row">
            <input
              class="field mono"
              bind:value={form.path}
              placeholder="/home/bender/Projects/mooi"
              oninput={() => (detected = null)}
            />
            <button
              type="button"
              class="btn"
              onclick={detectDirectory}
              disabled={detecting || !form.path.trim()}
            >
              {detecting ? "Checking…" : "Detect"}
            </button>
          </span>
        </label>
        <label class="wide-field">
          <span class="mono-label">Description <span class="faint">(optional)</span></span>
          <textarea class="field" bind:value={form.description} placeholder="Wallpaper engine, shell and shader experiments."></textarea>
        </label>
      </div>
      {#if detected}
        <p class="detected mono">
          {#if detected.isRepo}
            {detected.branch ?? "detached"} · {plural(detected.changed, "change")}{detected.ahead
              ? ` · ↑${detected.ahead}`
              : ""}{detected.behind ? ` · ↓${detected.behind}` : ""}{detected.github
              ? ` · ${detected.github}`
              : ""}
          {:else}
            {detected.error ?? "Not a git repository — the directory can still be linked."}
          {/if}
        </p>
      {/if}
      {#if formError}
        <p class="error">{formError}</p>
      {/if}
      <div class="form-foot">
        <span class="mono-label">{session.online ? "stored in local sqlite" : "backend offline"}</span>
        <div class="row">
          <button type="button" class="btn ghost" onclick={() => { creating = false; resetForm(); }}>Discard</button>
          <button type="submit" class="btn primary" disabled={submitting || !session.online}>
            {submitting ? "Creating…" : "Create project"}
          </button>
        </div>
      </div>
    </form>
  {/if}

  {#if loading}
    <p class="state mono">reading projects…</p>
  {:else if loadError}
    <div class="panel notice">
      <p class="tone-danger">Projects could not be read.</p>
      <p class="faint">{loadError}</p>
      <button class="btn" onclick={load}>Retry</button>
    </div>
  {:else if projects.length === 0}
    <div class="panel empty">
      <Icon name="folder-git" size={22} />
      <p class="empty-title">No projects yet</p>
      <p class="empty-copy">
        Start with a local directory, then attach its GitHub remote. Project-level themes,
        terminals and AI providers are configured here as those modules land.
      </p>
      <button class="btn" onclick={() => (creating = true)}>
        <Icon name="plus" size={13} /> Create the first project
      </button>
    </div>
  {:else}
    <div class="list-head">
      <span class="mono-label">{plural(projects.length, "project")}</span>
      <div class="head-actions">
        <span class="mono-label">branch · changes · remote</span>
        <button class="btn ghost" onclick={() => refreshGit()}>Refresh git</button>
      </div>
    </div>
    <ul class="list">
      {#each projects as project (project.id)}
        <li class="panel project">
          <div class="identity">
            <span class="glyph"><Icon name="folder-git" size={15} /></span>
            <div class="text">
              <span class="name">{project.name}</span>
              {#if project.description}<span class="desc">{project.description}</span>{/if}
            </div>
          </div>
          <div class="meta">
            <span class="path mono" class:unset={!project.path} title={project.path || "No directory linked"}>
              {project.path || "no directory linked"}
            </span>
            {#if project.path}
              {@const status = gitInfo(project.id)}
              {#if gitStates[project.id] === "loading"}
                <span class="git mono">reading git…</span>
              {:else if status?.isRepo}
                <span class="git mono">
                  <span class="branch" title={status.toplevel ?? project.path}>
                    <Icon name="git-branch" size={10} />{status.branch ?? "detached"}
                  </span>
                  {#if status.changed}<span class="dirty">{plural(status.changed, "change")}</span>{/if}
                  {#if status.ahead}<span class="ahead">↑{status.ahead}</span>{/if}
                  {#if status.behind}<span class="behind">↓{status.behind}</span>{/if}
                  {#if status.github}<span class="slug">{status.github}</span>{/if}
                </span>
              {:else if status?.error}
                <span class="git mono faint" title={status.error}>{status.path}</span>
              {:else if status}
                <span class="git mono faint">not a git repository</span>
              {/if}
            {:else}
              <span class="git mono faint">no git state · <span title={absoluteTime(project.createdAt)}>{relativeTime(project.createdAt)}</span></span>
            {/if}
          </div>
          <div class="actions">
            {#if confirmId === project.id}
              <span class="confirm mono-label">delete?</span>
              <button class="btn danger" onclick={() => remove(project.id)}>Confirm</button>
              <button class="btn ghost" onclick={() => (confirmId = null)}>Cancel</button>
            {:else}
              <button
                class="btn ghost icon"
                title="Delete project record"
                aria-label={`Delete ${project.name}`}
                onclick={() => (confirmId = project.id)}
              >
                <Icon name="trash" size={13} />
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
    <p class="footnote mono-label">
      Deleting a project removes its GRAFT record only. Files on disk are never touched.
    </p>
  {/if}
</section>

<style>
  .view-head {
    align-items: flex-start;
  }

  .view-head .view-sub {
    max-width: 78ch;
  }

  .form {
    margin-bottom: var(--gap);
    padding: 0 12px 10px;
  }

  .form-head {
    height: 30px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    margin-bottom: 10px;
  }

  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .wide-field {
    grid-column: span 2;
  }

  .error {
    margin-top: 9px;
    font-size: 10.5px;
    color: var(--danger);
  }

  .form-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 10px;
    padding-top: 9px;
    border-top: 1px solid var(--border);
  }

  .row {
    display: flex;
    gap: 6px;
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 14px;
    font-size: var(--font-small);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    padding: 26px 20px;
    color: var(--text-faint);
  }

  .empty-title {
    font-size: var(--font-base);
    font-weight: 600;
    color: var(--text);
  }

  .empty-copy {
    max-width: 62ch;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .empty .btn {
    margin-top: 6px;
  }

  .list-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px 6px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .project {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1.4fr) auto;
    align-items: center;
    gap: 14px;
    padding: 9px 10px 9px 12px;
    transition: border-color var(--motion);
  }

  .project:hover {
    border-color: var(--border-strong);
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .glyph {
    display: inline-flex;
    color: var(--text-faint);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .name {
    font-size: var(--font-small);
    font-weight: 600;
  }

  .desc {
    font-size: 10.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .path {
    font-size: 9.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path.unset {
    color: var(--text-faint);
    font-style: italic;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .head-actions .btn {
    height: 21px;
    padding: 0 6px;
    font-size: 10px;
  }

  .path-row {
    display: flex;
    gap: 5px;
  }

  .path-row .field {
    flex: 1;
    min-width: 0;
  }

  .detected {
    margin-top: 8px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-left: 2px solid var(--accent);
    border-radius: var(--radius);
    background: var(--bg-elev);
    font-size: 10px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .git {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 9px;
    color: var(--text-faint);
    overflow: hidden;
    white-space: nowrap;
  }

  .branch {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-muted);
  }

  .dirty {
    color: var(--warn);
  }

  .ahead {
    color: var(--ok);
  }

  .behind {
    color: var(--info);
  }

  .slug {
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .btn.danger {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    color: var(--danger);
    background: transparent;
    height: 22px;
  }

  .footnote {
    margin-top: 10px;
    font-size: 9.5px;
  }

  @media (max-width: 900px) {
    .project {
      grid-template-columns: minmax(0, 1fr) auto;
    }
    .meta {
      display: none;
    }
    .fields {
      grid-template-columns: 1fr;
    }
    .wide-field {
      grid-column: span 1;
    }
  }
</style>
