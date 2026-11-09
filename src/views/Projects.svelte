<script lang="ts">
  import { onMount } from "svelte";
  import { api, type GitActionResult, type GitBranch, type GitStatus, type Project, type ProjectEdge, type ProjectGraph } from "../lib/api";
  import { absoluteTime, plural, relativeTime } from "../lib/format";
  import { session } from "../lib/state/session.svelte";
  import { notify } from "../lib/state/ui.svelte";
  import { pushNotification } from "../lib/state/notifications.svelte";
  import Icon from "../lib/components/Icon.svelte";

  let projects = $state<Project[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let graph = $state<ProjectGraph | null>(null);
  let graphLoading = $state(false);
  let graphError = $state<string | null>(null);

  let creating = $state(false);
  let submitting = $state(false);
  let formError = $state<string | null>(null);
  let form = $state({ name: "", path: "", description: "" });

  let confirmId = $state<number | null>(null);
  let gitStates = $state<Record<number, GitStatus | "loading">>({});
  let detecting = $state(false);
  let detected = $state<GitStatus | null>(null);
  let gitBusy = $state<Record<number, "fetch" | "pull" | "push" | null>>({});
  const gitVerbs = ["fetch", "pull", "push"] as const;

  type GitPanel = "commit" | "branch";
  let gitPanel = $state<Record<number, GitPanel | null>>({});
  let branchList = $state<Record<number, GitBranch[]>>({});
  let branchLoading = $state<Record<number, boolean>>({});
  let commitDraft = $state<Record<number, string>>({});
  let stageAll = $state<Record<number, boolean>>({});
  let newBranch = $state<Record<number, string>>({});
  let switchNew = $state<Record<number, boolean>>({});
  let branchPick = $state<Record<number, string>>({});
  let writeBusy = $state<Record<number, string | null>>({});
  let pendingConfirm = $state<string | null>(null);
  let gitOutput = $state<Record<number, GitActionResult | null>>({});

  async function load(): Promise<void> {
    loading = true;
    loadError = null;
    try {
      projects = await api.listProjects();
      void refreshGit();
      void loadGraph();
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

  async function loadGraph(): Promise<void> {
    if (graphLoading) return;
    graphLoading = true;
    graphError = null;
    try {
      graph = await api.projectGraph();
    } catch (cause) {
      graphError = cause instanceof Error ? cause.message : String(cause);
    } finally {
      graphLoading = false;
    }
  }

  function edgeKind(kind: ProjectEdge["kind"]): string {
    return kind === "shared-remote" ? "shared remote" : kind === "depends-on" ? "depends on" : kind;
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

  async function runGit(project: Project, action: "fetch" | "pull" | "push"): Promise<void> {
    if (gitBusy[project.id]) return;
    gitBusy = { ...gitBusy, [project.id]: action };
    try {
      const result = await api.gitAction(project.path, action);
      const detail = (result.ok ? result.output : (result.error ?? result.output))
        .split("\n")
        .map((line) => line.trim())
        .find(Boolean);
      if (result.ok) {
        pushNotification({
          title: `Git ${action} · ${project.name}`,
          detail: detail ?? project.path,
          tone: "ok",
          target: { view: "projects" },
        });
      } else {
        notify(
          `${action} · ${project.name}${detail ? ` — ${detail}` : ""}`,
          "danger",
        );
      }
      await refreshGit();
    } finally {
      gitBusy = { ...gitBusy, [project.id]: null };
    }
  }

  function togglePanel(project: Project, panel: GitPanel): void {
    const next = gitPanel[project.id] === panel ? null : panel;
    gitPanel = { ...gitPanel, [project.id]: next };
    pendingConfirm = null;
    if (next === "commit") {
      if (commitDraft[project.id] === undefined) {
        commitDraft = { ...commitDraft, [project.id]: "" };
      }
      if (stageAll[project.id] === undefined) {
        stageAll = { ...stageAll, [project.id]: true };
      }
    }
    if (next === "branch") void loadBranchesFor(project);
  }

  async function loadBranchesFor(project: Project): Promise<void> {
    branchLoading = { ...branchLoading, [project.id]: true };
    try {
      const list = await api.gitBranches(project.path);
      branchList = { ...branchList, [project.id]: list };
      const current = list.find((item) => item.current);
      if (!branchPick[project.id] && current) {
        branchPick = { ...branchPick, [project.id]: current.name };
      }
    } catch (cause) {
      notify(cause instanceof Error ? cause.message : String(cause), "danger");
    } finally {
      branchLoading = { ...branchLoading, [project.id]: false };
    }
  }

  function remember(project: Project, result: GitActionResult): GitActionResult {
    gitOutput = { ...gitOutput, [project.id]: result };
    return result;
  }

  async function stageEverything(project: Project): Promise<void> {
    if (writeBusy[project.id]) return;
    writeBusy = { ...writeBusy, [project.id]: "stage" };
    try {
      const result = remember(project, await api.gitStage(project.path));
      notify(
        result.ok ? `Staged all changes in ${project.name}` : result.error ?? "Staging failed",
        result.ok ? "ok" : "danger",
      );
      if (result.ok) await refreshGit();
    } finally {
      writeBusy = { ...writeBusy, [project.id]: null };
    }
  }

  async function commitChanges(project: Project): Promise<void> {
    const message = (commitDraft[project.id] ?? "").trim();
    if (!message) {
      notify("Write a commit message first.", "danger");
      return;
    }
    if (writeBusy[project.id]) return;
    const key = `${project.id}:commit`;
    if (pendingConfirm !== key) {
      pendingConfirm = key;
      return;
    }
    pendingConfirm = null;
    writeBusy = { ...writeBusy, [project.id]: "commit" };
    try {
      const result = remember(
        project,
        await api.gitCommit(project.path, message, stageAll[project.id] ?? true),
      );
      if (result.ok) {
        commitDraft = { ...commitDraft, [project.id]: "" };
        pushNotification({
          title: `Committed · ${project.name}`,
          detail: message.split("\n")[0],
          tone: "ok",
          target: { view: "projects" },
        });
        await refreshGit();
        void loadBranchesFor(project);
      } else {
        notify(result.error ?? "Commit failed", "danger");
      }
    } finally {
      writeBusy = { ...writeBusy, [project.id]: null };
    }
  }

  async function createBranch(project: Project): Promise<void> {
    const name = (newBranch[project.id] ?? "").trim();
    if (!name) {
      notify("Name the new branch first.", "danger");
      return;
    }
    if (writeBusy[project.id]) return;
    const key = `${project.id}:branch`;
    if (pendingConfirm !== key) {
      pendingConfirm = key;
      return;
    }
    pendingConfirm = null;
    writeBusy = { ...writeBusy, [project.id]: "branch" };
    try {
      const result = remember(
        project,
        await api.gitCreateBranch(project.path, name, switchNew[project.id] ?? true),
      );
      if (result.ok) {
        newBranch = { ...newBranch, [project.id]: "" };
        pushNotification({
          title: `Branch created · ${name}`,
          detail: project.name,
          tone: "ok",
          target: { view: "projects" },
        });
        await refreshGit();
        void loadBranchesFor(project);
      } else {
        notify(result.error ?? "Branch creation failed", "danger");
      }
    } finally {
      writeBusy = { ...writeBusy, [project.id]: null };
    }
  }

  async function switchBranch(project: Project): Promise<void> {
    const name = branchPick[project.id];
    if (!name) return;
    if (writeBusy[project.id]) return;
    const key = `${project.id}:switch`;
    if (pendingConfirm !== key) {
      pendingConfirm = key;
      return;
    }
    pendingConfirm = null;
    writeBusy = { ...writeBusy, [project.id]: "switch" };
    try {
      const result = remember(project, await api.gitSwitchBranch(project.path, name));
      if (result.ok) {
        pushNotification({
          title: `Checked out · ${name}`,
          detail: project.name,
          tone: "ok",
          target: { view: "projects" },
        });
        await refreshGit();
        void loadBranchesFor(project);
      } else {
        notify(result.error ?? "Branch switch failed", "danger");
      }
    } finally {
      writeBusy = { ...writeBusy, [project.id]: null };
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
            {#if gitInfo(project.id)?.isRepo}
              <div class="git-actions">
                {#each gitVerbs as verb}
                  <button
                    class="btn ghost git-btn"
                    title={`git ${verb}`}
                    disabled={!!gitBusy[project.id]}
                    onclick={() => runGit(project, verb)}
                  >
                    {gitBusy[project.id] === verb ? "…" : verb}
                  </button>
                {/each}
                <button
                  class="btn ghost git-btn"
                  class:active={gitPanel[project.id] === "commit"}
                  title="Stage and commit"
                  onclick={() => togglePanel(project, "commit")}
                >commit</button>
                <button
                  class="btn ghost git-btn"
                  class:active={gitPanel[project.id] === "branch"}
                  title="Branches"
                  onclick={() => togglePanel(project, "branch")}
                >branch</button>
              </div>
            {/if}
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

          {#if gitPanel[project.id]}
            <div class="git-panel">
              {#if gitPanel[project.id] === "commit"}
                <div class="panel-head">
                  <span class="mono-label">commit</span>
                  <span class="mono faint">
                    {gitInfo(project.id)?.branch ?? "no branch"} ·
                    {plural(gitInfo(project.id)?.changed ?? 0, "change")}
                  </span>
                </div>
                <textarea
                  class="field commit-msg"
                  placeholder="Commit message"
                  value={commitDraft[project.id] ?? ""}
                  oninput={(event) => (commitDraft = { ...commitDraft, [project.id]: event.currentTarget.value })}
                ></textarea>
                <div class="panel-row">
                  <label class="check mono">
                    <input
                      type="checkbox"
                      checked={stageAll[project.id] ?? true}
                      onchange={(event) =>
                        (stageAll = { ...stageAll, [project.id]: event.currentTarget.checked })}
                    />
                    stage all changes
                  </label>
                  <button
                    class="btn ghost"
                    onclick={() => stageEverything(project)}
                    disabled={!!writeBusy[project.id]}
                  >
                    {writeBusy[project.id] === "stage" ? "Staging…" : "Stage only"}
                  </button>
                  <button
                    class="btn primary"
                    onclick={() => commitChanges(project)}
                    disabled={!!writeBusy[project.id]}
                  >
                    {writeBusy[project.id] === "commit"
                      ? "Committing…"
                      : pendingConfirm === `${project.id}:commit`
                        ? "Confirm commit"
                        : "Commit"}
                  </button>
                  {#if pendingConfirm === `${project.id}:commit`}
                    <button class="btn ghost" onclick={() => (pendingConfirm = null)}>Cancel</button>
                  {/if}
                </div>
                {#if pendingConfirm === `${project.id}:commit`}
                  <p class="confirm-note mono">
                    Will commit to {gitInfo(project.id)?.branch ?? "the current branch"}.
                  </p>
                {/if}
              {:else}
                <div class="panel-head">
                  <span class="mono-label">branches</span>
                  <span class="mono faint">{branchLoading[project.id] ? "reading…" : plural(branchList[project.id]?.length ?? 0, "branch")}</span>
                </div>
                <div class="panel-row">
                  <select
                    class="field mono branch-select"
                    value={branchPick[project.id] ?? ""}
                    onchange={(event) => (branchPick = { ...branchPick, [project.id]: event.currentTarget.value })}
                  >
                    <option value="">choose a branch…</option>
                    {#each branchList[project.id] ?? [] as item (item.name)}
                      <option value={item.name}>{item.name}{item.current ? " (current)" : ""}</option>
                    {/each}
                  </select>
                  <button
                    class="btn"
                    onclick={() => switchBranch(project)}
                    disabled={!!writeBusy[project.id] || !branchPick[project.id]}
                  >
                    {pendingConfirm === `${project.id}:switch` ? "Confirm switch" : "Switch"}
                  </button>
                  {#if pendingConfirm === `${project.id}:switch`}
                    <button class="btn ghost" onclick={() => (pendingConfirm = null)}>Cancel</button>
                  {/if}
                </div>
                <div class="panel-row">
                  <input
                    class="field mono branch-name"
                    placeholder="new-branch-name"
                    value={newBranch[project.id] ?? ""}
                    oninput={(event) => (newBranch = { ...newBranch, [project.id]: event.currentTarget.value })}
                  />
                  <label class="check mono">
                    <input
                      type="checkbox"
                      checked={switchNew[project.id] ?? true}
                      onchange={(event) =>
                        (switchNew = { ...switchNew, [project.id]: event.currentTarget.checked })}
                    />
                    switch to it
                  </label>
                  <button
                    class="btn primary"
                    onclick={() => createBranch(project)}
                    disabled={!!writeBusy[project.id]}
                  >
                    {writeBusy[project.id] === "branch"
                      ? "Creating…"
                      : pendingConfirm === `${project.id}:branch`
                        ? "Confirm create"
                        : "Create branch"}
                  </button>
                  {#if pendingConfirm === `${project.id}:branch`}
                    <button class="btn ghost" onclick={() => (pendingConfirm = null)}>Cancel</button>
                  {/if}
                </div>
              {/if}
              {#if gitOutput[project.id]}
                <pre class="git-out mono">{gitOutput[project.id]?.output || gitOutput[project.id]?.error}</pre>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
    <div class="relations panel">
      <div class="relations-head">
        <span class="mono-label">relationships</span>
        <div class="row">
          {#if graph}
            <span class="mono-label faint">
              {plural(graph.nodes.length, "project")} · {plural(graph.edges.length, "edge")}
            </span>
          {/if}
          <button class="btn ghost" onclick={loadGraph} disabled={graphLoading}>
            {graphLoading ? "Scanning…" : "Scan"}
          </button>
        </div>
      </div>
      {#if graphError}
        <p class="state tone-danger">{graphError}</p>
      {:else if graphLoading && !graph}
        <p class="state mono">reading remotes, manifests and extraction runs…</p>
      {:else if graph}
        {#if graph.edges.length === 0}
          <p class="state">
            No shared remotes, extraction runs or local package dependencies connect these projects.
          </p>
        {:else}
          <ul class="edges">
            {#each graph.edges as edge (edge.source + "-" + edge.target + "-" + edge.kind)}
              {@const from = graph.nodes.find((item) => item.id === edge.source)}
              {@const to = graph.nodes.find((item) => item.id === edge.target)}
              {#if from && to}
                <li class="edge">
                  <span class="name mono">{from.name}</span>
                  <span class="kind mono">{edgeKind(edge.kind)}</span>
                  <span class="arrow mono">→</span>
                  <span class="name mono">{to.name}</span>
                  <span class="detail mono faint" title={edge.label}>{edge.label}</span>
                </li>
              {/if}
            {/each}
          </ul>
        {/if}
        {#if graph.warnings.length > 0}
          <ul class="graph-warnings">
            {#each graph.warnings as warning}
              <li class="mono faint">{warning}</li>
            {/each}
          </ul>
        {/if}
      {/if}
    </div>
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
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 5px;
  }

  .git-actions {
    display: flex;
    gap: 3px;
  }

  .git-btn {
    height: 20px;
    padding: 0 6px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .git-btn.active {
    border-color: var(--border-strong);
    color: var(--accent);
  }

  .git-panel {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin-top: 3px;
    padding: 8px 9px;
    border-top: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-elev) 40%, transparent);
    border-radius: var(--radius);
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .commit-msg {
    min-height: 52px;
    resize: vertical;
    font-size: 10.5px;
    line-height: 1.55;
  }

  .panel-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .panel-row .btn {
    height: 22px;
    padding: 0 8px;
    font-size: 10px;
  }

  .branch-select,
  .branch-name {
    max-width: 240px;
    height: 24px;
    font-size: 10px;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9.5px;
    color: var(--text-faint);
    cursor: pointer;
  }

  .confirm-note {
    font-size: 9.5px;
    color: var(--warn);
  }

  .git-out {
    margin: 0;
    padding: 6px 8px;
    max-height: 110px;
    overflow: auto;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 9px;
    line-height: 1.5;
    color: var(--text-faint);
    white-space: pre-wrap;
  }

  .btn.danger {
    border-color: color-mix(in srgb, var(--danger) 50%, transparent);
    color: var(--danger);
    background: transparent;
    height: 22px;
  }

  .relations {
    margin-top: 8px;
    padding: 0 10px 9px;
  }

  .relations-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    height: 30px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 7px;
  }

  .edges {
    display: flex;
    flex-direction: column;
  }

  .edge {
    display: grid;
    grid-template-columns: auto auto auto auto minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    font-size: 10px;
  }

  .edge:last-child {
    border-bottom: 0;
  }

  .edge .name {
    color: var(--text);
  }

  .edge .kind {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 8.5px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .edge .arrow {
    color: var(--accent);
  }

  .edge .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 9px;
  }

  .graph-warnings {
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 9px;
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
