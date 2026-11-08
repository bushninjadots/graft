<script lang="ts">
  import { onMount } from "svelte";
  import { api, type GithubRepo, type SavedSearch } from "../lib/api";
  import { plural, relativeTime } from "../lib/format";
  import {
    github,
    isSignedIn,
    loadGithub,
    openRepo,
    searchRepos,
    type RepoSort,
  } from "../lib/state/github.svelte";
  import {
    addRepoToCollection,
    library,
    loadCollections,
  } from "../lib/state/library.svelte";
  import { navigate, notify } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  const TOPICS = [
    { id: "hyprland", label: "Hyprland" },
    { id: "ricing", label: "Ricing" },
    { id: "linux", label: "Linux" },
    { id: "arch", label: "Arch" },
    { id: "dotfiles", label: "Dotfiles" },
    { id: "wayland", label: "Wayland" },
    { id: "nixos", label: "NixOS" },
    { id: "rust", label: "Rust" },
  ];

  let saved = $state<SavedSearch[]>([]);
  let savedLoading = $state(false);
  let savingSearch = $state(false);
  let saveName = $state("");
  let saveOpen = $state(false);
  let deleteSearchId = $state<number | null>(null);
  let adding = $state<string | null>(null);

  const signedIn = $derived(isSignedIn());
  const discover = github.discover;

  onMount(() => {
    if (!github.status) void loadGithub();
    void loadSaved();
    void loadCollections();
  });

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  async function loadSaved(): Promise<void> {
    savedLoading = true;
    try {
      saved = await api.savedSearches();
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      savedLoading = false;
    }
  }

  function open(url: string | null | undefined): void {
    if (!url) return;
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    void searchRepos(true);
  }

  function pickTopic(topic: string): void {
    discover.topic = discover.topic === topic ? "" : topic;
    void searchRepos(true);
  }

  function reset(): void {
    discover.query = "";
    discover.language = "";
    discover.topic = "";
    discover.author = "";
    discover.sort = "best";
    discover.results = [];
    discover.totalCount = 0;
    discover.page = 1;
    discover.hasMore = false;
    discover.error = null;
    discover.searched = false;
    saveOpen = false;
  }

  async function saveCurrent(): Promise<void> {
    if (savingSearch) return;
    const name = (saveName.trim() || discover.query.trim() || discover.topic || discover.language || discover.author).slice(0, 80);
    if (!name) {
      notify("Search GitHub before saving it.", "danger");
      return;
    }
    savingSearch = true;
    try {
      const created = await api.saveSearch(
        name,
        discover.query,
        discover.language,
        discover.topic,
        discover.author,
        discover.sort,
      );
      saved = [created, ...saved];
      saveName = "";
      saveOpen = false;
      notify(`Saved search “${created.name}”`, "ok");
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      savingSearch = false;
    }
  }

  function applySaved(search: SavedSearch): void {
    discover.query = search.query;
    discover.language = search.language;
    discover.topic = search.topic;
    discover.author = search.author;
    discover.sort = (["best", "stars", "forks", "updated"].includes(search.sort)
      ? search.sort
      : "best") as RepoSort;
    discover.searched = true;
    void searchRepos(true);
  }

  async function deleteSaved(search: SavedSearch): Promise<void> {
    try {
      await api.deleteSavedSearch(search.id);
      saved = saved.filter((item) => item.id !== search.id);
      deleteSearchId = null;
      notify(`Deleted saved search “${search.name}”`, "ok");
    } catch (cause) {
      notify(message(cause), "danger");
    }
  }

  async function collect(repo: GithubRepo, collectionId: number): Promise<void> {
    const owner = repo.owner?.login;
    if (!owner) return;
    const key = `${repo.id}:${collectionId}`;
    adding = key;
    await addRepoToCollection(collectionId, owner, repo.name);
    adding = null;
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Explore</h1>
      <p class="view-sub">
        Search public GitHub by keyword, language, topic and author, keep the searches that matter,
        and file useful repositories into local collections.
      </p>
    </div>
    <div class="head-actions">
      <span class="badge">{library.collections.length} collection{library.collections.length === 1 ? "" : "s"}</span>
      <button class="btn ghost" onclick={() => navigate("collections")}>
        <Icon name="collections" size={13} />
        Open Collections
      </button>
    </div>
  </div>

  {#if !signedIn}
    <div class="panel notice">
      <div>
        <span class="badge warn">GitHub required</span>
        <p>Public search still uses the GitHub API. Connect an account to run queries.</p>
      </div>
      <button class="btn" onclick={() => navigate("repositories")}>Connect GitHub</button>
    </div>
  {/if}

  <div class="explore-layout">
    <aside class="panel saved">
      <div class="side-head">
        <span class="mono-label">saved searches</span>
        <span class="mono faint">{saved.length}</span>
      </div>
      <div class="saved-list">
        {#each saved as search (search.id)}
          <div class="saved-item">
            <button class="saved-main" onclick={() => applySaved(search)} title="Run this saved search">
              <span class="saved-name">{search.name}</span>
              <span class="saved-filters mono">
                {[
                  search.query,
                  search.language ? `lang:${search.language}` : "",
                  search.topic ? `#${search.topic}` : "",
                  search.author ? `@${search.author}` : "",
                ].filter(Boolean).join(" · ") || "all repositories"}
              </span>
              <span class="mono faint">{relativeTime(search.createdAt)}</span>
            </button>
            {#if deleteSearchId === search.id}
              <div class="saved-confirm">
                <span class="mono-label">delete?</span>
                <button class="btn ghost" onclick={() => void deleteSaved(search)}>Yes</button>
                <button class="btn ghost" onclick={() => (deleteSearchId = null)}>No</button>
              </div>
            {:else}
              <button class="btn ghost icon" title="Delete saved search" aria-label={`Delete ${search.name}`} onclick={() => (deleteSearchId = search.id)}>
                <Icon name="trash" size={11} />
              </button>
            {/if}
          </div>
        {:else}
          <p class="side-empty">
            {savedLoading ? "Reading saved searches…" : "No saved searches yet. Run a query and save it."}
          </p>
        {/each}
      </div>
      <div class="side-note">
        <span class="mono-label">collections</span>
        {#if library.collections.length === 0}
          <p class="hint">Create a collection to start a local library of repositories with notes and status.</p>
          <button class="btn ghost" onclick={() => navigate("collections")}>Create a collection</button>
        {:else}
          <p class="hint">Use Collect on a result to add it to one of your local collections.</p>
          <button class="btn ghost" onclick={() => navigate("collections")}>Manage collections</button>
        {/if}
      </div>
    </aside>

    <div class="results-column">
      <div class="panel search-panel">
        <form class="search-form" onsubmit={submit}>
          <Icon name="search" size={14} />
          <input
            class="search"
            bind:value={discover.query}
            placeholder="Search GitHub — hyprland, ricing, linux, dotfiles…"
            spellcheck="false"
            aria-label="Search public repositories"
          />
          <button class="btn primary" type="submit" disabled={discover.loading || !signedIn}>
            {discover.loading ? "Searching…" : "Search"}
          </button>
        </form>
        <div class="filters">
          <label class="field-label">
            <span class="mono faint">language</span>
            <input class="field" bind:value={discover.language} placeholder="Rust, QML, TypeScript" list="explore-languages" />
          </label>
          <datalist id="explore-languages">
            {#each ["Rust", "TypeScript", "Python", "QML", "C++", "Go", "Shell", "Lua", "Nix"] as language}
              <option value={language}></option>
            {/each}
          </datalist>
          <label class="field-label">
            <span class="mono faint">topic</span>
            <input class="field" bind:value={discover.topic} placeholder="hyprland, ricing…" spellcheck="false" />
          </label>
          <label class="field-label">
            <span class="mono faint">author</span>
            <input class="field" bind:value={discover.author} placeholder="login or @login" spellcheck="false" />
          </label>
          <label class="field-label">
            <span class="mono faint">sort</span>
            <select class="field" bind:value={discover.sort} onchange={() => discover.searched && void searchRepos(true)}>
              <option value="best">Best match</option>
              <option value="stars">Most stars</option>
              <option value="forks">Most forks</option>
              <option value="updated">Recently updated</option>
            </select>
          </label>
        </div>
        <div class="topics">
          <span class="mono-label">topics</span>
          {#each TOPICS as topic}
            <button class="chip" class:active={discover.topic === topic.id} onclick={() => pickTopic(topic.id)}>{topic.label}</button>
          {/each}
          <span class="spacer"></span>
          {#if discover.searched}
            <button class="link" onclick={() => (saveOpen = !saveOpen)}>Save this search</button>
          {/if}
          <button class="link" onclick={reset}>Reset</button>
        </div>
        {#if saveOpen}
          <div class="save-row">
            <input class="field" bind:value={saveName} maxlength="80" placeholder="Name this search" aria-label="Saved search name" />
            <button class="btn" onclick={() => void saveCurrent()} disabled={savingSearch}>
              {savingSearch ? "Saving…" : "Save search"}
            </button>
            <button class="btn ghost" onclick={() => (saveOpen = false)}>Cancel</button>
          </div>
        {/if}
      </div>

      {#if discover.error}
        <div class="panel notice">
          <div>
            <span class="badge warn">search failed</span>
            <p>{discover.error}</p>
          </div>
          <button class="btn" onclick={() => void searchRepos(true)}>Retry</button>
        </div>
      {:else if discover.loading && discover.results.length === 0}
        <p class="state mono">searching public repositories on api.github.com…</p>
      {:else if !discover.searched}
        <div class="panel empty">
          <Icon name="explore" size={24} />
          <p class="empty-title">Explore beyond your own repositories</p>
          <p class="empty-copy">
            Search by keyword, language, topic or author. Quick topics cover Hyprland, ricing,
            Linux, Arch, dotfiles, Wayland, NixOS and Rust. Every result is live GitHub data.
          </p>
        </div>
      {:else if discover.results.length === 0}
        <p class="state">No public repository matched this search.</p>
      {/if}

      {#if discover.results.length > 0}
        <div class="result-meta">
          <span class="mono-label">{plural(discover.totalCount, "result")} for GitHub search</span>
          <span class="mono faint">sorted by {discover.sort}</span>
        </div>
        <ul class="results">
          {#each discover.results as repo (repo.id)}
            <li class="panel result">
              <div class="result-main">
                <div class="result-title">
                  <Icon name="folder-git" size={14} />
                  <span class="repo-name">{repo.name}</span>
                  <span class="mono owner">{repo.owner?.login ?? "—"}</span>
                  <span class="badge">{repo.private ? "private" : "public"}</span>
                  {#if repo.archived}<span class="badge warn">archived</span>{/if}
                </div>
                {#if repo.description}<p class="repo-desc">{repo.description}</p>{/if}
                <div class="repo-facts mono">
                  <span>{repo.language ?? "no language"}</span>
                  {#each (repo.topics ?? []).slice(0, 4) as topic}<span class="topic">#{topic}</span>{/each}
                  <span>★ {repo.stargazersCount}</span>
                  <span>Y {repo.forksCount}</span>
                  <span>updated {relativeTime(Date.parse(repo.updatedAt) / 1000)}</span>
                </div>
              </div>
              <div class="result-actions">
                {#if library.collections.length > 0}
                  <details class="collect">
                    <summary class="btn ghost">Collect</summary>
                    <div class="collect-menu panel">
                      <span class="mono-label">add to collection</span>
                      {#each library.collections as collection (collection.id)}
                        <button
                          class="collect-option"
                          onclick={() => void collect(repo, collection.id)}
                          disabled={adding === `${repo.id}:${collection.id}`}
                        >
                          <span>{collection.name}</span>
                          <span class="mono faint">{collection.repoCount}</span>
                        </button>
                      {/each}
                    </div>
                  </details>
                {:else}
                  <button class="btn ghost" onclick={() => navigate("collections")}>Collect</button>
                {/if}
                <button class="btn ghost icon" title="Browse in GRAFT" aria-label={`Browse ${repo.fullName} in GRAFT`} onclick={() => openRepo(repo)}>
                  <Icon name="chevron-right" size={13} />
                </button>
                <button class="btn ghost icon" title="Open on GitHub" aria-label={`Open ${repo.fullName} on GitHub`} onclick={() => open(repo.htmlUrl)}>
                  <Icon name="arrow-up-right" size={13} />
                </button>
              </div>
            </li>
          {/each}
        </ul>
        {#if discover.hasMore}
          <button class="btn load-more" onclick={() => void searchRepos(false)} disabled={discover.loading}>
            {discover.loading ? "Loading…" : "Load more results"}
          </button>
        {/if}
      {/if}
    </div>
  </div>
</section>

<style>
  .head-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    margin-bottom: var(--gap);
    font-size: var(--font-small);
  }

  .notice p {
    margin-top: 2px;
    color: var(--text-muted);
  }

  .explore-layout {
    display: grid;
    grid-template-columns: 255px minmax(0, 1fr);
    gap: var(--gap);
    align-items: start;
  }

  .saved {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 170px);
    overflow: hidden;
  }

  .side-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 9px 10px;
    border-bottom: 1px solid var(--border);
  }

  .saved-list {
    overflow-y: auto;
    flex: 1;
  }

  .saved-item {
    display: flex;
    align-items: flex-start;
    gap: 2px;
    padding: 6px 5px 6px 9px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .saved-item:hover {
    background: var(--bg-elev);
  }

  .saved-main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .saved-name {
    font-size: 10.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .saved-filters {
    font-size: 8.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .saved-item > .btn {
    width: 21px;
    height: 21px;
  }

  .saved-confirm {
    display: flex;
    align-items: center;
    gap: 1px;
    flex-wrap: wrap;
    justify-content: flex-end;
    max-width: 94px;
  }

  .saved-confirm .btn {
    height: 19px;
    padding: 0 4px;
    font-size: 8px;
  }

  .saved-confirm .mono-label {
    font-size: 7.5px;
  }

  .side-empty,
  .side-note {
    padding: 10px;
  }

  .side-empty {
    font-size: 10px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .side-note {
    border-top: 1px solid var(--border);
    background: var(--bg-elev);
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .side-note .btn {
    align-self: flex-start;
  }

  .hint {
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .search-panel {
    padding: 8px 9px;
    margin-bottom: 7px;
  }

  .search-form {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    color: var(--text-faint);
  }

  .search {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--font-small);
    outline: none;
  }

  .search::placeholder {
    color: var(--text-faint);
  }

  .search-form .btn {
    height: 23px;
    font-size: 9.5px;
  }

  .filters {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 5px;
    margin: 5px 0 7px;
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .field {
    height: 23px;
    font-size: 9.5px;
  }

  .topics {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    height: 20px;
    padding: 0 7px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text-faint);
    font-size: 8.5px;
    cursor: pointer;
  }

  .chip.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }

  .spacer {
    flex: 1;
  }

  .link {
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .link:hover {
    color: var(--accent);
  }

  .save-row {
    display: flex;
    gap: 5px;
    margin-top: 7px;
    padding-top: 7px;
    border-top: 1px solid var(--border);
  }

  .save-row .field {
    flex: 1;
  }

  .save-row .btn {
    height: 23px;
    font-size: 9.5px;
  }

  .result-meta {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 5px 2px;
  }

  .results {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .result {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 8px 8px 11px;
  }

  .result:hover {
    border-color: var(--border-strong);
  }

  .result-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .result-title {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-faint);
  }

  .repo-name {
    font-size: var(--font-small);
    font-weight: 600;
    color: var(--text);
  }

  .owner {
    font-size: 9px;
    color: var(--text-faint);
  }

  .repo-desc {
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100ch;
  }

  .repo-facts {
    display: flex;
    gap: 10px;
    font-size: 9px;
    color: var(--text-faint);
    overflow: hidden;
    white-space: nowrap;
  }

  .topic {
    color: color-mix(in srgb, var(--accent) 75%, var(--text-faint));
  }

  .result-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }

  .result-actions .btn {
    height: 23px;
    font-size: 9px;
    padding: 0 6px;
  }

  .collect {
    position: relative;
  }

  .collect summary {
    list-style: none;
    cursor: pointer;
  }

  .collect summary::-webkit-details-marker {
    display: none;
  }

  .collect-menu {
    position: absolute;
    top: 25px;
    right: 0;
    z-index: 10;
    width: 190px;
    padding: 5px;
    box-shadow: 0 12px 28px -12px var(--shadow);
  }

  .collect-option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    width: 100%;
    padding: 5px 4px;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 10px;
    text-align: left;
    cursor: pointer;
  }

  .collect-option:hover {
    background: var(--bg-elev);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    padding: 24px 18px;
    color: var(--text-faint);
  }

  .empty-title {
    font-size: var(--font-base);
    font-weight: 600;
    color: var(--text);
  }

  .empty-copy {
    max-width: 68ch;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
    padding: 4px 1px;
  }

  .load-more {
    margin-top: 7px;
  }

  @media (max-width: 1080px) {
    .explore-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .saved {
      max-height: 240px;
    }
  }

  @media (max-width: 760px) {
    .filters {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
