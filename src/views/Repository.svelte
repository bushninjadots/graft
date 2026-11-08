<script lang="ts">
  import { onMount } from "svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import {
    api,
    type GithubRepoDetail,
    type RepoBranch,
    type RepoCommit,
    type RepoCommitDetail,
    type RepoEntry,
    type RepoFile,
    type RepoPull,
    type RepoPullDetail,
  } from "../lib/api";
  import { closeRepo, github } from "../lib/state/github.svelte";
  import { notify } from "../lib/state/ui.svelte";
  import { plural, relativeTime } from "../lib/format";
  import Icon from "../lib/components/Icon.svelte";

  type Tab = "overview" | "code" | "commits" | "pulls" | "branches";

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

  let branches = $state<RepoBranch[]>([]);
  let branch = $state<string | null>(null);
  let commits = $state<RepoCommit[]>([]);
  let commitsLoading = $state(false);
  let commitPage = $state(1);
  let commitHasMore = $state(false);
  let commitDetail = $state<RepoCommitDetail | null>(null);
  let commitLoading = $state(false);

  let pullState = $state<"open" | "closed" | "all">("open");
  let pulls = $state<RepoPull[]>([]);
  let pullsLoading = $state(false);
  let pullDetail = $state<RepoPullDetail | null>(null);
  let pullLoading = $state(false);

  let starBusy = $state(false);
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
      await Promise.all([loadDir([]), loadBranches(), loadCommits(true), loadPulls()]);
    } catch (cause) {
      error = message(cause);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void load();
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

  async function toggleStar(): Promise<void> {
    if (!detail) return;
    starBusy = true;
    try {
      const starred = await api.setStar(owner, name, !detail.starred);
      detail = { ...detail, starred };
      notify(starred ? "Starred" : "Star removed", "ok");
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
      notify(`Fork created: ${forked.fullName}`, "ok");
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
    try {
      file = await api.repoFile(owner, name, path, branch);
    } catch (cause) {
      codeError = message(cause);
    } finally {
      fileLoading = false;
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
    try {
      pullDetail = await api.repoPull(owner, name, pull.number);
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      pullLoading = false;
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
        <button class="btn ghost" onclick={() => open(detail?.htmlUrl ?? repo.htmlUrl)}>GitHub ↗</button>
        <button class="btn ghost" onclick={toggleStar} disabled={starBusy || !detail}>
          {detail?.starred ? "★ Starred" : "☆ Star"}
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
        {#each [["overview", "Overview"], ["code", "Code"], ["commits", "Commits"], ["pulls", "Pull requests"], ["branches", "Branches"]] as [id, label]}
          <button
            class="tab"
            class:active={tab === id}
            role="tab"
            aria-selected={tab === id}
            onclick={() => (tab = id as Tab)}
          >
            {label}
            {#if id === "pulls" && pulls.length}<span class="count">{pulls.length}</span>{/if}
            {#if id === "branches" && branches.length}<span class="count">{branches.length}</span>{/if}
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
              <select class="field mono" bind:value={branch} onchange={() => { file = null; void loadDir(dirPath); }}>
                {#each branches as item (item.name)}
                  <option value={item.name}>{item.name}</option>
                {/each}
              </select>
            </div>
          </div>

          {#if file}
            <div class="file-head">
              <button class="link" onclick={() => (file = null)}>← back to {dirPath.join("/") || name}</button>
              <span class="mono faint">{size(file.size)} · {file.path}</span>
            </div>
            {#if file.content !== null}
              <pre class="code">{file.content}</pre>
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
      {:else if tab === "commits"}
        <div class="panel split">
          <div class="bar">
            <div class="branch-pick">
              <Icon name="git-branch" size={11} />
              <select class="field mono" bind:value={branch} onchange={() => loadCommits(true)}>
                {#each branches as item (item.name)}
                  <option value={item.name}>{item.name}</option>
                {/each}
              </select>
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
                {#each commitDetail.files as changed (changed.filename)}
                  <div class="diff-file">
                    <div class="diff-head mono">
                      <span>{changed.filename}</span>
                      <span class="add">+{changed.additions}</span>
                      <span class="del">−{changed.deletions}</span>
                    </div>
                    {#if changed.patch}
                      <pre class="patch">{changed.patch}</pre>
                    {:else}
                      <p class="hint">No textual diff (binary or too large).</p>
                    {/if}
                  </div>
                {/each}
              {/if}
            </div>
          {:else if commitsLoading && commits.length === 0}
            <p class="state mono">reading commits…</p>
          {:else if commits.length === 0}
            <p class="state">No commits on {branch ?? "this branch"}.</p>
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
                {#each pullDetail.files as changed (changed.filename)}
                  <div class="diff-file">
                    <div class="diff-head mono">
                      <span>{changed.filename}</span>
                      <span class="add">+{changed.additions}</span>
                      <span class="del">−{changed.deletions}</span>
                    </div>
                    {#if changed.patch}
                      <pre class="patch">{changed.patch}</pre>
                    {:else}
                      <p class="hint">No textual diff (binary or too large).</p>
                    {/if}
                  </div>
                {/each}
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

  .diff-file {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .diff-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 4px 8px;
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
    font-size: 9.5px;
  }

  .diff-head span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .add {
    color: var(--ok);
  }

  .del {
    color: var(--danger);
  }

  .patch {
    margin: 0;
    padding: 7px 9px;
    font-family: var(--font-mono);
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-muted);
    overflow-x: auto;
    white-space: pre;
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
