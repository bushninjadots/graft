<script lang="ts">
  import { onMount } from "svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import {
    api,
    type GithubRepo,
    type GithubRepoDetail,
    type RepoBranch,
    type RepoCommit,
    type RepoCommitDetail,
    type RepoEntry,
    type RepoFile,
    type RepoIssue,
    type RepoPull,
    type RepoPullComment,
    type RepoPullDetail,
    type RepoPullReview,
    type RepoRelease,
  } from "../lib/api";
  import { closeRepo, github, loadRepos } from "../lib/state/github.svelte";
  import { notify, ui } from "../lib/state/ui.svelte";
  import { pushNotification, type NotificationTarget } from "../lib/state/notifications.svelte";
  import {
    addWatch,
    isWatched,
    loadWatchtower,
    watchtower,
  } from "../lib/state/watchtower.svelte";
  import { plural, relativeTime } from "../lib/format";
  import Icon from "../lib/components/Icon.svelte";
  import DiffView from "../lib/components/DiffView.svelte";
  import BranchGraph from "../lib/components/BranchGraph.svelte";

  type Tab = "overview" | "code" | "commits" | "pulls" | "issues" | "branches" | "releases";

  const repo = $derived(github.selected);
  const owner = $derived(repo?.owner?.login ?? repo?.fullName.split("/")[0] ?? "");
  const name = $derived(repo?.name ?? "");

  let tab = $state<Tab>("overview");
  let detail = $state<GithubRepoDetail | null>(null);
  let readme = $state<RepoFile | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let dirPath = $state<string[]>([]);
  let entries = $state<RepoEntry[]>([]);
  let entriesLoading = $state(false);
  let codeError = $state<string | null>(null);
  let file = $state<RepoFile | null>(null);
  let fileLoading = $state(false);
  let editing = $state(false);
  let editContent = $state("");
  let editMessage = $state("");
  let editConfirm = $state(false);
  let editSaving = $state(false);
  let editError = $state<string | null>(null);
  const canEdit = $derived(!!detail?.permissions?.push);

  let branches = $state<RepoBranch[]>([]);
  let branch = $state<string | null>(null);
  let releases = $state<RepoRelease[]>([]);
  let releasesLoading = $state(false);
  let commits = $state<RepoCommit[]>([]);
  let commitsLoading = $state(false);
  let commitPage = $state(1);
  let commitHasMore = $state(false);
  let commitDetail = $state<RepoCommitDetail | null>(null);
  let commitLoading = $state(false);
  let commitView = $state<"list" | "graph">("list");

  let pullState = $state<"open" | "closed" | "all">("open");
  let pulls = $state<RepoPull[]>([]);
  let pullsLoading = $state(false);
  let pullDetail = $state<RepoPullDetail | null>(null);
  let pullLoading = $state(false);
  let pullReviews = $state<RepoPullReview[]>([]);
  let pullComments = $state<RepoPullComment[]>([]);
  let reviewEvent = $state<"APPROVE" | "REQUEST_CHANGES" | "COMMENT">("COMMENT");
  let reviewBody = $state("");
  let reviewConfirm = $state(false);
  let reviewSaving = $state(false);
  let reviewError = $state<string | null>(null);
  let commentTarget = $state<{ path: string; line: number; side: "LEFT" | "RIGHT" } | null>(null);
  let commentBody = $state("");
  let commentSaving = $state(false);
  let commentError = $state<string | null>(null);

  let issues = $state<RepoIssue[]>([]);
  let issuesLoading = $state(false);
  let issueState = $state<"open" | "closed" | "all">("open");
  let newIssueOpen = $state(false);
  let issueTitle = $state("");
  let issueBody = $state("");
  let issueConfirm = $state(false);
  let issueSaving = $state(false);
  let issueError = $state<string | null>(null);

  let refreshing = $state(false);
  let lastRefresh = 0;

  let starBusy = $state(false);
  let watchBusy = $state(false);
  const watching = $derived(!!detail && isWatched(detail.fullName));
  let forkBusy = $state(false);
  let forked = $state<{ fullName: string; htmlUrl: string } | null>(null);
  let cloneOpen = $state(false);
  let clonePath = $state("");
  let cloning = $state(false);
  let cloneError = $state<string | null>(null);
  let clonedPath = $state<string | null>(null);
  let addingProject = $state(false);

  let readmeEl = $state<HTMLDivElement | null>(null);

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function open(url: string | null | undefined): void {
    if (!url) return;
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  function render(source: string): string {
    const parsed = marked.parse(source, { async: false, gfm: true });
    return DOMPurify.sanitize(typeof parsed === "string" ? parsed : "");
  }

  async function load(): Promise<void> {
    if (!repo) return;
    loading = true;
    error = null;
    try {
      const [info, readmeFile] = await Promise.all([
        api.repoDetail(owner, name),
        api.repoReadme(owner, name).catch(() => null),
      ]);
      detail = info;
      readme = readmeFile;
      branch = info.defaultBranch;
      clonePath = await api.cloneDestination(owner, name).catch(() => "");
      await Promise.all([loadDir([]), loadBranches(), loadCommits(true), loadPulls(), loadIssues(), loadReleases()]);
      const focus = ui.repositoryFocus;
      ui.repositoryFocus = null;
      if (focus) {
        tab = focus.tab as Tab;
        if (focus.path) {
          await openFile(focus.path);
          tab = "code";
        }
      }
    } catch (cause) {
      error = message(cause);
    } finally {
      loading = false;
      lastRefresh = Date.now();
    }
  }

  async function refreshRepository(silent = false): Promise<void> {
    if (!repo || refreshing || loading || editing) return;
    refreshing = true;
    try {
      const [info, readmeFile] = await Promise.all([
        api.repoDetail(owner, name),
        api.repoReadme(owner, name).catch(() => null),
      ]);
      detail = info;
      readme = readmeFile;
      await loadBranches();
      if (tab === "code") {
        const openPath = file?.path ?? null;
        await loadDir(dirPath);
        if (openPath) await openFile(openPath);
      } else if (tab === "commits") {
        await loadCommits(true);
      } else if (tab === "pulls") {
        await loadPulls();
      } else if (tab === "issues") {
        await loadIssues();
      } else if (tab === "releases") {
        await loadReleases();
      }
      lastRefresh = Date.now();
      if (!silent) notify("Repository refreshed from GitHub", "ok");
    } catch (cause) {
      if (!silent) notify(message(cause), "danger");
    } finally {
      refreshing = false;
    }
  }

  onMount(() => {
    if (!watchtower.loaded) void loadWatchtower();
    void load();
    const onFocus = () => {
      if (!loading && !editing && Date.now() - lastRefresh > 60_000) {
        void refreshRepository(true);
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  // README links leave GRAFT through the Rust opener rather than navigating the webview.
  $effect(() => {
    const element = readmeEl;
    if (!element) return;
    const onClick = (event: MouseEvent) => {
      const anchor = (event.target as HTMLElement | null)?.closest("a");
      if (!anchor) return;
      event.preventDefault();
      const href = anchor.getAttribute("href");
      if (!href || href.startsWith("#")) return;
      let resolved = href;
      if (!/^https?:\/\//i.test(href)) {
        const base = detail?.htmlUrl ?? repo?.htmlUrl ?? "";
        const branchName = detail?.defaultBranch ?? "main";
        const dir = (readme?.path ?? "").split("/").slice(0, -1).join("/");
        resolved = href.startsWith("/")
          ? `https://github.com${href}`
          : `${base}/blob/${branchName}/${dir ? `${dir}/` : ""}${href}`;
      }
      void api.openExternal(resolved).catch((cause) => notify(message(cause), "danger"));
    };
    element.addEventListener("click", onClick);
    return () => element.removeEventListener("click", onClick);
  });

  async function watchThis(): Promise<void> {
    if (!detail || watchBusy || watching) return;
    watchBusy = true;
    const ok = await addWatch(owner, name);
    watchBusy = false;
    if (ok) pushNotification({
      title: "Repository added to Watchtower",
      detail: detail.fullName,
      tone: "ok",
      target: { view: "watchtower" },
    });
  }

  async function toggleStar(): Promise<void> {
    if (!detail) return;
    starBusy = true;
    try {
      const starred = await api.setStar(owner, name, !detail.starred);
      detail = { ...detail, starred };
      pushNotification({
        title: starred ? "Repository starred" : "Star removed",
        detail: `${owner}/${name}`,
        tone: "ok",
        target: repoTarget("overview"),
      });
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      starBusy = false;
    }
  }

  async function forkRepo(): Promise<void> {
    forkBusy = true;
    try {
      forked = await api.forkRepo(owner, name);
      void loadRepos(true);
      pushNotification({
        title: "Repository forked",
        detail: `${owner}/${name} → ${forked.fullName}`,
        tone: "ok",
        target: repoTarget("overview"),
      });
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      forkBusy = false;
    }
  }

  async function clone(): Promise<void> {
    if (!detail) return;
    cloning = true;
    cloneError = null;
    try {
      clonedPath = await api.cloneRepo(detail.cloneUrl, clonePath);
      notify(`Cloned to ${clonedPath}`, "ok");
    } catch (cause) {
      cloneError = message(cause);
    } finally {
      cloning = false;
    }
  }

  async function addAsProject(): Promise<void> {
    if (!detail || !clonedPath) return;
    addingProject = true;
    try {
      await api.createProject(detail.name, clonedPath, detail.description ?? "");
      notify(`Added “${detail.name}” to My Projects`, "ok");
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      addingProject = false;
    }
  }

  async function loadDir(target: string[]): Promise<void> {
    entriesLoading = true;
    codeError = null;
    file = null;
    try {
      entries = await api.repoContents(owner, name, target.join("/"), branch);
      dirPath = target;
    } catch (cause) {
      codeError = message(cause);
    } finally {
      entriesLoading = false;
    }
  }

  function enter(entry: RepoEntry): void {
    if (entry.type === "dir") void loadDir([...dirPath, entry.name]);
    else void openFile(entry.path);
  }

  async function openFile(path: string): Promise<void> {
    fileLoading = true;
    codeError = null;
    cancelEdit();
    try {
      file = await api.repoFile(owner, name, path, branch);
    } catch (cause) {
      codeError = message(cause);
    } finally {
      fileLoading = false;
    }
  }

  function startEdit(): void {
    if (!file || file.content === null) return;
    editContent = file.content;
    editMessage = `Update ${file.name}`;
    editConfirm = false;
    editError = null;
    editing = true;
  }

  function cancelEdit(): void {
    editing = false;
    editConfirm = false;
    editError = null;
    editContent = "";
  }

  async function commitEdit(): Promise<void> {
    if (!file || editSaving) return;
    editSaving = true;
    editError = null;
    try {
      const result = await api.repoSaveFile(
        owner,
        name,
        file.path,
        editContent,
        file.sha,
        branch,
        editMessage,
      );
      file = {
        ...file,
        content: editContent,
        sha: result.sha,
        size: new TextEncoder().encode(editContent).length,
      };
      editing = false;
      editConfirm = false;
      lastRefresh = Date.now();
      pushNotification({
        title: "File edited on GitHub",
        detail: `${owner}/${name}/${result.path} · ${branch}`,
        tone: "ok",
        target: repoTarget("code", file.path),
      });
      void loadCommits(true);
    } catch (cause) {
      editError = message(cause);
    } finally {
      editSaving = false;
    }
  }

  async function loadBranches(): Promise<void> {
    try {
      branches = await api.repoBranches(owner, name);
    } catch {
      branches = [];
    }
  }

  async function loadCommits(reset: boolean): Promise<void> {
    commitsLoading = true;
    commitDetail = null;
    try {
      const page = reset ? 1 : commitPage;
      const list = await api.repoCommits(owner, name, branch, page);
      commits = reset ? list : [...commits, ...list];
      commitPage = page + 1;
      commitHasMore = list.length === 40;
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      commitsLoading = false;
    }
  }

  function chooseBranch(next: string): void {
    branch = next;
    tab = "commits";
    void loadCommits(true);
  }

  async function openCommit(sha: string): Promise<void> {
    commitLoading = true;
    commitDetail = null;
    try {
      commitDetail = await api.repoCommit(owner, name, sha);
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      commitLoading = false;
    }
  }

  async function loadIssues(): Promise<void> {
    issuesLoading = true;
    try {
      issues = await api.repoIssues(owner, name, issueState);
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      issuesLoading = false;
    }
  }

  function setIssueState(next: "open" | "closed" | "all"): void {
    issueState = next;
    void loadIssues();
  }

  async function loadReleases(): Promise<void> {
    releasesLoading = true;
    try {
      releases = await api.repoReleases(owner, name);
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      releasesLoading = false;
    }
  }

  function resetIssueForm(): void {
    newIssueOpen = false;
    issueTitle = "";
    issueBody = "";
    issueConfirm = false;
    issueError = null;
  }

  async function createIssue(): Promise<void> {
    if (issueSaving) return;
    issueSaving = true;
    issueError = null;
    try {
      const created = await api.createIssue(owner, name, issueTitle, issueBody);
      resetIssueForm();
      await loadIssues();
      pushNotification({
        title: `Issue #${created.number} created`,
        detail: `${created.title} · ${owner}/${name}`,
        tone: "ok",
        target: repoTarget("issues"),
      });
    } catch (cause) {
      issueError = message(cause);
    } finally {
      issueSaving = false;
    }
  }

  function repoTarget(tabName: string, path?: string): NotificationTarget {
    return {
      view: "repository",
      repo: repo ? ($state.snapshot(repo) as GithubRepo) : null,
      tab: tabName,
      path,
    };
  }

  async function loadPulls(): Promise<void> {
    pullsLoading = true;
    pullDetail = null;
    try {
      pulls = await api.repoPulls(owner, name, pullState);
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      pullsLoading = false;
    }
  }

  function setPullState(next: "open" | "closed" | "all"): void {
    pullState = next;
    void loadPulls();
  }

  async function openPull(pull: RepoPull): Promise<void> {
    pullLoading = true;
    pullDetail = null;
    pullReviews = [];
    pullComments = [];
    resetReview();
    try {
      const [detail, reviews, comments] = await Promise.all([
        api.repoPull(owner, name, pull.number),
        api.repoPullReviews(owner, name, pull.number).catch(() => []),
        api.repoPullComments(owner, name, pull.number).catch(() => []),
      ]);
      pullDetail = detail;
      pullReviews = reviews;
      pullComments = comments;
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      pullLoading = false;
    }
  }

  function reviewActionLabel(event: "APPROVE" | "REQUEST_CHANGES" | "COMMENT"): string {
    return event === "APPROVE"
      ? "Approve"
      : event === "REQUEST_CHANGES"
        ? "Request changes"
        : "Comment";
  }

  function resetReview(): void {
    reviewEvent = "COMMENT";
    reviewBody = "";
    reviewConfirm = false;
    reviewError = null;
    commentTarget = null;
    commentBody = "";
    commentError = null;
  }

  async function submitReview(): Promise<void> {
    if (!pullDetail || reviewSaving) return;
    reviewSaving = true;
    reviewError = null;
    try {
      await api.submitPullReview(
        owner,
        name,
        pullDetail.pull.number,
        reviewEvent,
        reviewBody,
      );
      pullReviews = await api.repoPullReviews(owner, name, pullDetail.pull.number);
      const action = reviewActionLabel(reviewEvent);
      resetReview();
      pushNotification({
        title: `${action} submitted`,
        detail: `#${pullDetail.pull.number} ${pullDetail.pull.title}`,
        tone: "ok",
        target: repoTarget("pulls"),
      });
    } catch (cause) {
      reviewError = message(cause);
    } finally {
      reviewSaving = false;
    }
  }

  function beginPullComment(path: string, line: number, side: "LEFT" | "RIGHT"): void {
    commentTarget = { path, line, side };
    commentBody = "";
    commentError = null;
  }

  async function submitPullComment(): Promise<void> {
    if (!pullDetail || !commentTarget || commentSaving) return;
    commentSaving = true;
    commentError = null;
    try {
      const created = await api.createPullComment(
        owner,
        name,
        pullDetail.pull.number,
        commentBody,
        pullDetail.pull.headSha,
        commentTarget.path,
        commentTarget.line,
        commentTarget.side,
      );
      const targetPath = commentTarget.path;
      pullComments = [...pullComments, created];
      commentTarget = null;
      commentBody = "";
      pushNotification({
        title: "Inline review comment posted",
        detail: `${targetPath} · #${pullDetail.pull.number}`,
        tone: "ok",
        target: repoTarget("pulls"),
      });
    } catch (cause) {
      commentError = message(cause);
    } finally {
      commentSaving = false;
    }
  }

</script>

<section class="view">
  {#if !repo}
    <div class="panel notice">
      <p>No repository selected.</p>
      <button class="btn" onclick={closeRepo}>Back to repositories</button>
    </div>
  {:else}
    <div class="repo-head">
      <button class="btn ghost icon back" title="All repositories" aria-label="All repositories" onclick={closeRepo}>
        <Icon name="chevron-left" size={14} />
      </button>
      <div class="titles">
        <div class="crumbs mono">
          <span class="faint">{owner}</span><span class="faint">/</span><span>{name}</span>
          {#if detail?.private}<span class="badge">private</span>{/if}
          {#if detail?.fork}<span class="badge">fork</span>{/if}
          {#if detail?.archived}<span class="badge warn">archived</span>{/if}
        </div>
        <p class="desc">{detail?.description ?? "No description on GitHub."}</p>
      </div>
      <div class="head-actions">
        <button class="btn ghost" onclick={() => void refreshRepository()} disabled={refreshing || loading}>
          <span class:spin={refreshing}><Icon name="refresh" size={12} /></span>
          {refreshing ? "Refreshing…" : "Refresh"}
        </button>
        <button class="btn ghost" onclick={() => open(detail?.htmlUrl ?? repo.htmlUrl)}>GitHub ↗</button>
        <button class="btn ghost" onclick={toggleStar} disabled={starBusy || !detail}>
          {detail?.starred ? "★ Starred" : "☆ Star"}
        </button>
        <button class="btn ghost" onclick={watchThis} disabled={watchBusy || !detail || watching}>
          {watching ? "Watching" : watchBusy ? "Adding…" : "Watch"}
        </button>
        <button class="btn ghost" onclick={forkRepo} disabled={forkBusy}>
          {forkBusy ? "Forking…" : "Fork"}
        </button>
        <button class="btn primary" onclick={() => (cloneOpen = !cloneOpen)} disabled={!detail}>
          {cloning ? "Cloning…" : "Clone / download"}
        </button>
      </div>
    </div>

    {#if cloneOpen && detail}
      <div class="panel clone">
        <div class="clone-row">
          <span class="mono-label">clone to</span>
          <input class="field mono" bind:value={clonePath} spellcheck="false" />
          <button class="btn" onclick={clone} disabled={cloning || !clonePath.trim()}>
            {cloning ? "Cloning…" : "Clone"}
          </button>
        </div>
        <p class="hint">
          Uses the Git CLI over HTTPS. Cloning to an existing folder is refused. Remotes:
          <span class="mono">{detail.cloneUrl}</span>
        </p>
        {#if cloneError}<p class="error">{cloneError}</p>{/if}
        {#if clonedPath}
          <div class="cloned">
            <span class="mono">cloned → {clonedPath}</span>
            <button class="btn ghost" onclick={addAsProject} disabled={addingProject}>
              {addingProject ? "Adding…" : "Add to My Projects"}
            </button>
          </div>
        {/if}
        {#if forked}
          <p class="hint">
            Fork: <span class="mono">{forked.fullName}</span>
            <button class="link" onclick={() => open(forked?.htmlUrl)}>open on GitHub ↗</button>
          </p>
        {/if}
      </div>
    {/if}

    {#if loading}
      <p class="state mono">reading {owner}/{name} from api.github.com…</p>
    {:else if error}
      <div class="panel notice">
        <p class="tone-danger">This repository could not be read.</p>
        <p class="faint">{error}</p>
        <div class="row">
          <button class="btn" onclick={load}>Retry</button>
          <button class="btn ghost" onclick={closeRepo}>Back</button>
        </div>
      </div>
    {:else if detail}
      <div class="facts panel">
        <span><strong>{detail.stargazersCount}</strong> stars</span>
        <span><strong>{detail.forksCount}</strong> forks</span>
        <span><strong>{detail.watchersCount}</strong> watchers</span>
        <span><strong>{detail.openIssuesCount}</strong> open issues</span>
        <span><strong>{size(detail.size * 1024)}</strong></span>
        <span>{detail.language ?? "no language"}</span>
        {#if detail.license}<span>{detail.license.spdxId ?? detail.license.name}</span>{/if}
        <span class="faint">updated {relativeTime(Date.parse(detail.pushedAt ?? detail.updatedAt) / 1000)}</span>
      </div>

      {#if detail.topics.length > 0}
        <div class="topics">
          {#each detail.topics as topic}
            <span class="topic mono">{topic}</span>
          {/each}
        </div>
      {/if}

      <div class="tabs" role="tablist">
        {#each [["overview", "Overview"], ["code", "Code"], ["commits", "Commits"], ["pulls", "Pull requests"], ["issues", "Issues"], ["branches", "Branches"], ["releases", "Releases"]] as [id, label]}
          <button
            class="tab"
            class:active={tab === id}
            role="tab"
            aria-selected={tab === id}
            onclick={() => (tab = id as Tab)}
          >
            {label}
            {#if id === "pulls" && pulls.length}<span class="count">{pulls.length}</span>{/if}
            {#if id === "issues" && issues.length}<span class="count">{issues.length}</span>{/if}
            {#if id === "branches" && branches.length}<span class="count">{branches.length}</span>{/if}
            {#if id === "releases" && releases.length}<span class="count">{releases.length}</span>{/if}
          </button>
        {/each}
      </div>

      {#if tab === "overview"}
        <div class="panel readme">
          {#if readme?.content}
            <div class="readme-head mono-label">
              {readme.path} <span class="faint">· {size(readme.size)}</span>
            </div>
            <div class="markdown" bind:this={readmeEl}>{@html render(readme.content)}</div>
          {:else if readme?.truncated}
            <p class="state">The README is too large to render here. <button class="link" onclick={() => open(readme?.htmlUrl)}>Open on GitHub ↗</button></p>
          {:else}
            <p class="state">This repository has no README.</p>
          {/if}
        </div>
      {:else if tab === "code"}
        <div class="panel code-panel">
          <div class="code-bar">
            <div class="crumbs mono">
              <button class="link" onclick={() => loadDir([])}>{name}</button>
              {#each dirPath as segment, index}
                <span class="faint">/</span>
                <button class="link" onclick={() => loadDir(dirPath.slice(0, index + 1))}>{segment}</button>
              {/each}
              {#if file}<span class="faint">/</span><span>{file.name}</span>{/if}
            </div>
            <div class="branch-pick">
              <Icon name="git-branch" size={11} />
              <select class="field mono" bind:value={branch} onchange={() => { file = null; cancelEdit(); void loadDir(dirPath); }}>
                {#each branches as item (item.name)}
                  <option value={item.name}>{item.name}</option>
                {/each}
              </select>
            </div>
          </div>

          {#if file}
            <div class="file-head">
              <button class="link" onclick={() => { file = null; cancelEdit(); }}>← back to {dirPath.join("/") || name}</button>
              <div class="file-actions">
                <span class="mono faint">{size(file.size)} · {file.path}</span>
                {#if file.content !== null && !file.binary && !file.truncated}
                  {#if canEdit}
                    {#if editing}
                      <span class="badge warn">editing</span>
                    {:else}
                      <button class="btn ghost" onclick={startEdit}>Edit</button>
                    {/if}
                  {:else}
                    <span class="mono faint" title="Your GitHub account does not report push access to this repository">read-only</span>
                  {/if}
                {/if}
              </div>
            </div>
            {#if file.content !== null}
              {#if editing}
                <div class="edit-wrap">
                  <textarea
                    class="edit-area mono"
                    bind:value={editContent}
                    spellcheck="false"
                    aria-label={`Edit ${file.path}`}
                  ></textarea>
                  <div class="edit-bar">
                    <span class="mono faint">
                      {editContent.split("\n").length} lines · {editContent.length} chars · {branch}
                    </span>
                    <div class="edit-actions">
                      <button class="btn ghost" onclick={cancelEdit} disabled={editSaving}>Cancel</button>
                      <button class="btn primary" onclick={() => (editConfirm = true)} disabled={editSaving || editContent === file.content}>
                        Review commit
                      </button>
                    </div>
                  </div>
                  {#if editConfirm}
                    <div class="commit-confirm">
                      <div class="commit-head">
                        <span class="mono-label">commit to github</span>
                        <span class="badge warn">external write · {branch}</span>
                      </div>
                      <p class="commit-copy">
                        This commits <span class="mono">{file.path}</span> directly to
                        <span class="mono">{owner}/{name}</span> on <span class="mono">{branch}</span>.
                        It bypasses local Git and cannot be undone from GRAFT.
                      </p>
                      <label class="commit-field">
                        <span class="mono-label">Commit message</span>
                        <input class="field" bind:value={editMessage} maxlength="240" />
                      </label>
                      {#if editError}<p class="error">{editError}</p>{/if}
                      <div class="edit-actions">
                        <button class="btn ghost" onclick={() => (editConfirm = false)} disabled={editSaving}>Back</button>
                        <button class="btn primary" onclick={() => void commitEdit()} disabled={editSaving}>
                          {editSaving ? "Committing…" : "Confirm commit"}
                        </button>
                      </div>
                    </div>
                  {:else if editError}
                    <p class="error">{editError}</p>
                  {/if}
                </div>
              {:else}
                <pre class="code">{file.content}</pre>
              {/if}
            {:else if file.binary}
              <p class="state">Binary file — GRAFT shows text only. <button class="link" onclick={() => open(file?.htmlUrl)}>Open on GitHub ↗</button></p>
            {:else}
              <p class="state">Preview unavailable for this file. <button class="link" onclick={() => open(file?.htmlUrl)}>Open on GitHub ↗</button></p>
            {/if}
          {:else if fileLoading}
            <p class="state mono">reading file…</p>
          {:else if codeError}
            <p class="error">{codeError}</p>
          {:else if entriesLoading}
            <p class="state mono">reading directory…</p>
          {:else}
            <ul class="entries">
              {#each entries as entry (entry.path)}
                <li>
                  <button class="entry" onclick={() => enter(entry)}>
                    <span class="kind mono">{entry.type === "dir" ? "dir" : entry.type === "submodule" ? "sub" : "file"}</span>
                    <span class="entry-name mono">{entry.name}</span>
                    <span class="entry-size mono">{entry.type === "file" ? size(entry.size) : ""}</span>
                  </button>
                </li>
              {:else}
                <li class="state">This directory is empty.</li>
              {/each}
            </ul>
          {/if}
        </div>
      {:else if tab === "issues"}
        <div class="panel issues-panel">
          <div class="bar">
            <div class="segmented">
              {#each [["open", "Open"], ["closed", "Closed"], ["all", "All"]] as [id, label]}
                <button class="seg" class:active={issueState === id} onclick={() => setIssueState(id as "open" | "closed" | "all")}>{label}</button>
              {/each}
            </div>
            <div class="row">
              <button class="btn ghost icon" title="Refresh issues" aria-label="Refresh issues" onclick={() => loadIssues()} disabled={issuesLoading}>
                <span class:spin={issuesLoading}><Icon name="refresh" size={12} /></span>
              </button>
              <button class="btn ghost" onclick={() => (newIssueOpen = !newIssueOpen)}>
                {newIssueOpen ? "Close form" : "New issue"}
              </button>
            </div>
          </div>

          {#if newIssueOpen}
            <div class="issue-form">
              <span class="mono-label">new issue · {owner}/{name}</span>
              <input class="field" bind:value={issueTitle} maxlength="256" placeholder="Issue title" aria-label="Issue title" />
              <textarea class="field issue-body-input" bind:value={issueBody} maxlength="60000" placeholder="Describe the issue (Markdown)" aria-label="Issue body"></textarea>
              {#if issueError}<p class="error">{issueError}</p>{/if}
              <div class="row">
                <button class="btn ghost" onclick={resetIssueForm} disabled={issueSaving}>Cancel</button>
                <button class="btn primary" onclick={() => (issueConfirm = true)} disabled={issueSaving || !issueTitle.trim()}>Review issue</button>
              </div>
              {#if issueConfirm}
                <div class="commit-confirm">
                  <div class="commit-head">
                    <span class="mono-label">create on github</span>
                    <span class="badge warn">external write</span>
                  </div>
                  <p class="commit-copy">This creates a public issue in <span class="mono">{owner}/{name}</span>. It cannot be removed from GRAFT.</p>
                  <div class="edit-actions">
                    <button class="btn ghost" onclick={() => (issueConfirm = false)} disabled={issueSaving}>Back</button>
                    <button class="btn primary" onclick={() => void createIssue()} disabled={issueSaving}>
                      {issueSaving ? "Creating…" : "Confirm issue"}
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}

          {#if issuesLoading && issues.length === 0}
            <p class="state mono">reading issues…</p>
          {:else if issues.length === 0}
            <p class="state">No {issueState === "all" ? "" : issueState} issues.</p>
          {:else}
            <div class="issues">
              {#each issues as issue (issue.number)}
                <details class="issue">
                  <summary>
                    <span class="issue-state" class:closed={issue.state === "closed"}>{issue.state === "closed" ? "●" : "○"}</span>
                    <span class="issue-title">{issue.title}</span>
                    <span class="mono faint">#{issue.number}</span>
                    {#each issue.labels as label}
                      <span class="issue-label">{label}</span>
                    {/each}
                    <span class="mono faint issue-meta">@{issue.authorLogin} · {relativeTime(Date.parse(issue.updatedAt) / 1000)}</span>
                  </summary>
                  {#if issue.body}
                    <pre class="issue-body">{issue.body}</pre>
                  {:else}
                    <p class="state">No description.</p>
                  {/if}
                  <button class="link" onclick={() => open(issue.htmlUrl)}>Open issue on GitHub ↗</button>
                </details>
              {/each}
            </div>
          {/if}
        </div>
      {:else if tab === "commits"}
        <div class="panel split">
          <div class="bar">
            <div class="row">
              <div class="branch-pick">
                <Icon name="git-branch" size={11} />
                <select class="field mono" bind:value={branch} onchange={() => loadCommits(true)}>
                  {#each branches as item (item.name)}
                    <option value={item.name}>{item.name}</option>
                  {/each}
                </select>
              </div>
              <div class="segmented">
                <button class="seg" class:active={commitView === "list"} onclick={() => (commitView = "list")}>List</button>
                <button class="seg" class:active={commitView === "graph"} onclick={() => (commitView = "graph")}>Graph</button>
              </div>
            </div>
            <button class="btn ghost icon" title="Refresh commits" aria-label="Refresh commits" onclick={() => loadCommits(true)} disabled={commitsLoading}>
              <span class:spin={commitsLoading}><Icon name="refresh" size={12} /></span>
            </button>
          </div>

          {#if commitDetail || commitLoading}
            <div class="commit-detail">
              <button class="link" onclick={() => (commitDetail = null)}>← all commits</button>
              {#if commitLoading}
                <p class="state mono">reading commit…</p>
              {:else if commitDetail}
                <h3 class="mono">{commitDetail.sha.slice(0, 7)} · {commitDetail.message.split("\n")[0]}</h3>
                <p class="hint">
                  {commitDetail.authorName}
                  {#if commitDetail.date}· {relativeTime(Date.parse(commitDetail.date) / 1000)}{/if}
                  · +{commitDetail.additions} −{commitDetail.deletions}
                  · <button class="link" onclick={() => open(commitDetail?.htmlUrl)}>open on GitHub ↗</button>
                </p>
                {#if commitDetail.message.split("\n").length > 1}
                  <pre class="message">{commitDetail.message.split("\n").slice(1).join("\n").trim()}</pre>
                {/if}
                <DiffView files={commitDetail.files} />
              {/if}
            </div>
          {:else if commitsLoading && commits.length === 0}
            <p class="state mono">reading commits…</p>
          {:else if commits.length === 0}
            <p class="state">No commits on {branch ?? "this branch"}.</p>
          {:else if commitView === "graph"}
            <BranchGraph {commits} onselect={openCommit} />
          {:else}
            <ul class="commits">
              {#each commits as commit (commit.sha)}
                <li>
                  <button class="commit" onclick={() => openCommit(commit.sha)}>
                    <span class="sha mono">{commit.short}</span>
                    <span class="msg">{commit.message}</span>
                    <span class="who mono faint">{commit.authorLogin ?? commit.authorName}</span>
                    <span class="when mono faint">{commit.date ? relativeTime(Date.parse(commit.date) / 1000) : ""}</span>
                  </button>
                </li>
              {/each}
            </ul>
            {#if commitHasMore}
              <button class="btn load-more" onclick={() => loadCommits(false)} disabled={commitsLoading}>
                {commitsLoading ? "Loading…" : "Load older commits"}
              </button>
            {/if}
          {/if}
        </div>
      {:else if tab === "pulls"}
        <div class="panel split">
          <div class="bar">
            <div class="segmented">
              {#each ["open", "closed", "all"] as option}
                <button class="seg" class:active={pullState === option} onclick={() => setPullState(option as "open" | "closed" | "all")}>{option}</button>
              {/each}
            </div>
            <button class="btn ghost icon" title="Refresh pull requests" aria-label="Refresh pull requests" onclick={loadPulls} disabled={pullsLoading}>
              <span class:spin={pullsLoading}><Icon name="refresh" size={12} /></span>
            </button>
          </div>

          {#if pullDetail || pullLoading}
            <div class="pull-detail">
              <button class="link" onclick={() => (pullDetail = null)}>← all pull requests</button>
              {#if pullLoading}
                <p class="state mono">reading pull request…</p>
              {:else if pullDetail}
                {@const pull = pullDetail.pull}
                <h3>#{pull.number} {pull.title}</h3>
                <p class="hint">
                  <span class="mono">{pull.headRef} → {pull.baseRef}</span>
                  · {pull.authorLogin}
                  · +{pull.additions} −{pull.deletions} in {plural(pull.changedFiles, "file")}
                  · <button class="link" onclick={() => open(detail?.htmlUrl ? `${detail.htmlUrl}/pull/${pull.number}` : pull.htmlUrl)}>open on GitHub ↗</button>
                </p>
                {#if pull.body}
                  <div class="markdown" bind:this={readmeEl}>{@html render(pull.body)}</div>
                {/if}

                {#if pullReviews.length > 0}
                  <div class="review-list">
                    <span class="mono-label">reviews</span>
                    {#each pullReviews as review (review.id)}
                      <div class="review-item">
                        <span class="badge" class:ok={review.state === "APPROVED"} class:warn={review.state === "CHANGES_REQUESTED"}>
                          {review.state.toLowerCase().replaceAll("_", " ")}
                        </span>
                        <span class="mono faint">@{review.authorLogin}</span>
                        {#if review.body}<p>{review.body}</p>{/if}
                      </div>
                    {/each}
                  </div>
                {/if}

                {#if pull.state === "open"}
                  <div class="review-box">
                    <div class="review-top">
                      <span class="mono-label">review this pull request</span>
                      <select class="field" bind:value={reviewEvent} disabled={reviewSaving || reviewConfirm}>
                        <option value="COMMENT">Comment</option>
                        <option value="APPROVE">Approve</option>
                        <option value="REQUEST_CHANGES">Request changes</option>
                      </select>
                    </div>
                    <textarea
                      class="field review-body"
                      bind:value={reviewBody}
                      maxlength="65536"
                      placeholder={reviewEvent === "APPROVE" ? "Optional approval note" : "Review summary (required)"}
                      aria-label="Review summary"
                      disabled={reviewSaving || reviewConfirm}
                    ></textarea>
                    {#if reviewError}<p class="error">{reviewError}</p>{/if}
                    {#if reviewConfirm}
                      <div class="review-confirm">
                        <span class="badge warn">external write · {reviewActionLabel(reviewEvent)}</span>
                        <p class="hint">
                          This submits a real review on #{pull.number} in {owner}/{name}. GitHub
                          records the reviewer identity from your token and cannot undo it from GRAFT.
                        </p>
                        <div class="row">
                          <button class="btn ghost" onclick={() => (reviewConfirm = false)} disabled={reviewSaving}>Back</button>
                          <button class="btn primary" onclick={() => void submitReview()} disabled={reviewSaving}>
                            {reviewSaving ? "Submitting…" : `Confirm ${reviewActionLabel(reviewEvent).toLowerCase()}`}
                          </button>
                        </div>
                      </div>
                    {:else}
                      <div class="review-actions">
                        <span class="hint">Select a diff line to leave an inline comment. Reviews and comments are external writes and ask for confirmation.</span>
                        <button
                          class="btn"
                          onclick={() => (reviewConfirm = true)}
                          disabled={reviewSaving || (reviewEvent !== "APPROVE" && !reviewBody.trim())}
                        >
                          {reviewActionLabel(reviewEvent)}
                        </button>
                      </div>
                    {/if}
                  </div>

                  {#if commentTarget}
                    <div class="inline-review-form">
                      <div class="review-top">
                        <span class="mono-label">inline comment</span>
                        <span class="mono faint">{commentTarget.path}:{commentTarget.line} · {commentTarget.side}</span>
                      </div>
                      <textarea
                        class="field review-body"
                        bind:value={commentBody}
                        maxlength="65536"
                        placeholder="Comment on this diff line…"
                        aria-label="Inline review comment"
                        disabled={commentSaving}
                      ></textarea>
                      {#if commentError}<p class="error">{commentError}</p>{/if}
                      <div class="review-actions">
                        <span class="hint">Posting targets commit {pull.headSha.slice(0, 7)}.</span>
                        <button class="btn ghost" onclick={() => (commentTarget = null)} disabled={commentSaving}>Cancel</button>
                        <button class="btn primary" onclick={() => void submitPullComment()} disabled={commentSaving || !commentBody.trim()}>
                          {commentSaving ? "Posting…" : "Post comment"}
                        </button>
                      </div>
                    </div>
                  {/if}
                {:else}
                  <p class="hint">This pull request is {pull.merged ? "merged" : pull.state}; new reviews and comments are disabled.</p>
                {/if}

                <DiffView
                  files={pullDetail.files}
                  comments={pullComments}
                  commentable={pull.state === "open"}
                  oncomment={beginPullComment}
                />
              {/if}
            </div>
          {:else if pullsLoading && pulls.length === 0}
            <p class="state mono">reading pull requests…</p>
          {:else if pulls.length === 0}
            <p class="state">No {pullState === "all" ? "" : pullState} pull requests.</p>
          {:else}
            <ul class="pulls">
              {#each pulls as pull (pull.number)}
                <li>
                  <button class="commit pull-row" onclick={() => openPull(pull)}>
                    <span class="pu mono">#{pull.number}</span>
                    <span class="msg">{pull.title}</span>
                    <span class="badge" class:warn={pull.state === "open"} class:ok={pull.merged}>{pull.merged ? "merged" : pull.state}</span>
                    <span class="who mono faint">{pull.authorLogin}</span>
                    <span class="when mono faint">{relativeTime(Date.parse(pull.updatedAt) / 1000)}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {:else if tab === "branches"}
        <div class="panel split">
          {#if branches.length === 0}
            <p class="state">No branches were returned.</p>
          {:else}
            <ul class="branches">
              {#each branches as item (item.name)}
                <li>
                  <button class="branch-row" onclick={() => chooseBranch(item.name)}>
                    <span class="mono"><Icon name="git-branch" size={11} /> {item.name}</span>
                    {#if item.protected}<span class="badge">protected</span>{/if}
                    <span class="mono faint">{item.sha.slice(0, 7)}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {:else if tab === "releases"}
        <div class="panel split">
          {#if releasesLoading && releases.length === 0}
            <p class="state mono">reading releases…</p>
          {:else if releases.length === 0}
            <p class="state">No releases have been published.</p>
          {:else}
            <ul class="releases">
              {#each releases as release (release.id)}
                <li class="release">
                  <div class="release-head">
                    <span class="mono tag">{release.tagName}</span>
                    {#if release.name}<span class="release-name">{release.name}</span>{/if}
                    {#if release.draft}<span class="badge">draft</span>{/if}
                    {#if release.prerelease}<span class="badge warn">prerelease</span>{/if}
                    <span class="mono faint when">
                      {release.publishedAt ? relativeTime(Date.parse(release.publishedAt) / 1000) : "unpublished"}
                    </span>
                  </div>
                  <p class="hint">
                    {release.authorLogin} · {release.targetCommitish}
                    {#if release.assetCount}
                      · {plural(release.assetCount, "asset")} · {release.downloadCount.toLocaleString()} downloads
                    {/if}
                    · <button class="link" onclick={() => open(release.htmlUrl)}>open release ↗</button>
                  </p>
                  {#if release.body}
                    <pre class="release-body">{release.body}</pre>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
    {/if}
  {/if}
</section>

<style>
  .repo-head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding-bottom: 10px;
  }

  .back {
    margin-top: 1px;
  }

  .titles {
    flex: 1;
    min-width: 0;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--font-small);
    font-weight: 600;
  }

  .desc {
    margin-top: 3px;
    font-size: 10.5px;
    color: var(--text-faint);
  }

  .head-actions {
    display: flex;
    gap: 5px;
    flex-shrink: 0;
  }

  .head-actions .btn {
    height: 24px;
    font-size: 10px;
    padding: 0 8px;
  }

  .clone {
    margin-bottom: var(--gap);
    padding: 10px 11px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .clone-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .clone-row .field {
    flex: 1;
    min-width: 0;
  }

  .cloned {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 10px;
    color: var(--text-muted);
  }

  .hint {
    font-size: 10px;
    color: var(--text-faint);
    line-height: 1.55;
  }

  .link {
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .link:hover {
    color: var(--accent);
  }

  .error {
    font-size: 10.5px;
    color: var(--danger);
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
    padding: 4px 1px;
  }

  .notice {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 14px;
  }

  .row {
    display: flex;
    gap: 6px;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 7px 11px;
    margin-bottom: 7px;
    font-size: 10px;
    color: var(--text-faint);
  }

  .facts strong {
    color: var(--text);
    font-weight: 600;
  }

  .topics {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 8px;
  }

  .topic {
    font-size: 9px;
    padding: 1px 5px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-faint);
  }

  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
    margin-bottom: var(--gap);
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 27px;
    padding: 0 9px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 10.5px;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .count {
    font-size: 9px;
    padding: 0 4px;
    border-radius: 999px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
  }

  .readme {
    padding: 0 14px 14px;
  }

  .readme-head {
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
    margin-bottom: 11px;
  }

  .readme :global(.markdown) {
    font-size: 11.5px;
    line-height: 1.65;
    color: var(--text-muted);
    max-width: 100ch;
  }

  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3) {
    color: var(--text);
    font-size: 14px;
    margin: 16px 0 7px;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
  }

  .markdown :global(p) {
    margin: 7px 0;
  }

  .markdown :global(a) {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .markdown :global(code) {
    font-family: var(--font-mono);
    font-size: 10.5px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 2px;
    padding: 0 3px;
  }

  .markdown :global(pre) {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 8px 10px;
    overflow-x: auto;
  }

  .markdown :global(pre code) {
    border: 0;
    background: none;
    padding: 0;
  }

  .markdown :global(img) {
    max-width: 100%;
  }

  .markdown :global(ul),
  .markdown :global(ol) {
    padding-left: 18px;
    margin: 6px 0;
  }

  .markdown :global(blockquote) {
    border-left: 2px solid var(--border-strong);
    margin: 8px 0;
    padding-left: 10px;
    color: var(--text-faint);
  }

  .markdown :global(table) {
    border-collapse: collapse;
    font-size: 10.5px;
  }

  .markdown :global(th),
  .markdown :global(td) {
    border: 1px solid var(--border);
    padding: 3px 7px;
  }

  .code-panel {
    padding: 0 0 8px;
  }

  .issues-panel {
    padding: 0 0 8px;
  }

  .issues {
    display: flex;
    flex-direction: column;
  }

  .issue {
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    padding: 6px 10px;
  }

  .issue summary {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
    list-style: none;
    min-width: 0;
  }

  .issue summary::-webkit-details-marker {
    display: none;
  }

  .issue-state {
    color: var(--ok);
    font-size: 10px;
  }

  .issue-state.closed {
    color: var(--danger);
  }

  .issue-title {
    font-size: 11px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issue-label {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 8.5px;
    color: var(--text-faint);
  }

  .issue-meta {
    margin-left: auto;
    white-space: nowrap;
  }

  .issue-body {
    margin: 8px 0 4px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.55;
    color: var(--text-muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .issue-form {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 9px 10px;
    border-bottom: 1px solid var(--border);
  }

  .issue-body-input {
    min-height: 90px;
    padding: 6px 8px;
    font-size: 10.5px;
    line-height: 1.5;
    resize: vertical;
  }

  .code-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 9px;
    border-bottom: 1px solid var(--border);
  }

  .branch-pick {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--text-faint);
  }

  .branch-pick .field {
    height: 21px;
    padding: 0 4px;
    font-size: 9.5px;
    max-width: 180px;
  }

  .entries {
    display: flex;
    flex-direction: column;
  }

  .entry {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 11px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: transparent;
    text-align: left;
    cursor: pointer;
    font: inherit;
    color: var(--text);
  }

  .entry:hover {
    background: var(--bg-elev);
  }

  .kind {
    font-size: 8.5px;
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .entry-name {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-size {
    font-size: 9px;
    color: var(--text-faint);
  }

  .file-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    font-size: 10px;
  }

  .code {
    margin: 0;
    padding: 9px 11px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-muted);
    overflow-x: auto;
  }

  .file-actions {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .file-actions .btn {
    height: 21px;
    padding: 0 8px;
    font-size: 9.5px;
  }

  .edit-wrap {
    display: flex;
    flex-direction: column;
  }

  .edit-area {
    min-height: 380px;
    max-height: 66vh;
    border: 0;
    border-radius: 0;
    padding: 9px 11px;
    background: var(--bg-elev);
    color: var(--text);
    font-size: 10.5px;
    line-height: 1.6;
    resize: vertical;
    tab-size: 2;
  }

  .edit-area:focus {
    outline: none;
  }

  .edit-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 9px;
    padding: 6px 9px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .edit-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .edit-actions .btn {
    height: 22px;
    padding: 0 8px;
    font-size: 10px;
  }

  .commit-confirm {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 10px 11px;
    border-top: 1px solid var(--border);
    border-left: 2px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 4%, transparent);
  }

  .commit-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .commit-copy {
    font-size: 10px;
    line-height: 1.55;
    color: var(--text-muted);
    max-width: 84ch;
  }

  .commit-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 460px;
  }

  .commit-field .field {
    height: 24px;
    font-size: 10.5px;
  }

  .split {
    padding: 0 0 8px;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 9px;
    border-bottom: 1px solid var(--border);
  }

  .segmented {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .seg {
    height: 21px;
    padding: 0 9px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9.5px;
    cursor: pointer;
    text-transform: capitalize;
  }

  .seg.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .commits,
  .pulls,
  .branches {
    display: flex;
    flex-direction: column;
  }

  .commit,
  .branch-row {
    display: grid;
    grid-template-columns: 62px minmax(0, 1fr) 96px 66px;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 5px 10px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .pull-row {
    grid-template-columns: 42px minmax(0, 1fr) auto 96px 66px;
  }

  .branch-row {
    grid-template-columns: minmax(0, 1fr) auto 66px;
  }

  .commit:hover,
  .branch-row:hover {
    background: var(--bg-elev);
  }

  .releases {
    display: flex;
    flex-direction: column;
  }

  .release {
    padding: 9px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .release:last-child {
    border-bottom: 0;
  }

  .release-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 7px;
  }

  .release-head .tag {
    color: var(--accent);
    font-size: 10px;
  }

  .release-name {
    font-size: 10.5px;
    font-weight: 600;
  }

  .release-head .when {
    margin-left: auto;
  }

  .release-body {
    margin: 7px 0 0;
    padding: 7px 9px;
    max-height: 180px;
    overflow: auto;
    border-left: 2px solid var(--border);
    background: var(--bg-elev);
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 9.5px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .sha,
  .pu {
    font-size: 9.5px;
    color: var(--accent);
  }

  .msg {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .who,
  .when {
    font-size: 9px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }

  .commit-detail,
  .pull-detail {
    padding: 9px 11px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .commit-detail h3,
  .pull-detail h3 {
    font-size: 11.5px;
    font-weight: 600;
  }

  .message {
    margin: 0;
    padding: 7px 9px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    white-space: pre-wrap;
  }

  .review-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .review-item {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 10px;
  }

  .review-item p {
    flex-basis: 100%;
    margin: 1px 0 0;
    color: var(--text-muted);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .review-box,
  .inline-review-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .inline-review-form {
    border-left: 2px solid var(--accent);
  }

  .review-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }

  .review-top .field {
    height: 22px;
    width: 145px;
    font-size: 9.5px;
  }

  .review-body {
    height: 54px;
    padding: 5px 7px;
    resize: vertical;
    font-size: 10px;
    line-height: 1.45;
  }

  .review-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    flex-wrap: wrap;
  }

  .review-actions .hint {
    flex: 1;
    min-width: 150px;
    text-align: left;
  }

  .review-confirm {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px;
    border: 1px dashed color-mix(in srgb, var(--warn) 45%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 5%, transparent);
  }

  .load-more {
    margin: 7px 10px 4px;
  }

  .mono {
    font-family: var(--font-mono);
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

  @media (max-width: 1100px) {
    .repo-head {
      flex-wrap: wrap;
    }
    .head-actions {
      width: 100%;
      justify-content: flex-end;
    }
    .commit,
    .pull-row {
      grid-template-columns: 52px minmax(0, 1fr) 66px;
    }
    .commit .who,
    .pull-row .badge,
    .pull-row .who {
      display: none;
    }
  }
</style>
