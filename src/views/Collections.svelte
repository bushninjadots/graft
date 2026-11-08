<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CollectionDetail, type CollectionRepo } from "../lib/api";
  import { plural, relativeTime } from "../lib/format";
  import {
    addRepoToCollection,
    createCollection,
    deleteCollection,
    library,
    loadCollection,
    loadCollections,
    removeCollectionRepo,
    updateCollectionRepo,
  } from "../lib/state/library.svelte";
  import { openRepoByCoords } from "../lib/state/github.svelte";
  import { navigate, notify } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  const STATUSES = [
    { id: "want_to_try", label: "Want to try" },
    { id: "studying", label: "Studying" },
    { id: "using", label: "Using" },
    { id: "contributing", label: "Contributing" },
    { id: "archived", label: "Archived" },
  ];

  let selectedId = $state<number | null>(null);
  let detail = $state<CollectionDetail | null>(null);
  let detailLoading = $state(false);
  let createOpen = $state(false);
  let name = $state("");
  let description = $state("");
  let creating = $state(false);
  let addOwner = $state("");
  let addName = $state("");
  let adding = $state(false);
  let addError = $state<string | null>(null);
  let deleteConfirm = $state(false);
  let removing = $state<number | null>(null);
  let saving = $state<number | null>(null);

  onMount(() => {
    void (async () => {
      await loadCollections();
      if (library.collections[0]) await select(library.collections[0].id);
    })();
  });

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function open(url: string): void {
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

  async function select(id: number): Promise<void> {
    selectedId = id;
    detailLoading = true;
    deleteConfirm = false;
    try {
      detail = await loadCollection(id);
    } finally {
      detailLoading = false;
    }
  }

  async function create(): Promise<void> {
    if (creating) return;
    creating = true;
    const created = await createCollection(name, description);
    creating = false;
    if (created) {
      name = "";
      description = "";
      createOpen = false;
      await select(created.id);
    }
  }

  async function removeCollection(): Promise<void> {
    if (!detail) return;
    const removed = await deleteCollection(detail.collection);
    if (!removed) return;
    deleteConfirm = false;
    detail = null;
    selectedId = library.collections[0]?.id ?? null;
    if (selectedId) await select(selectedId);
  }

  function parseRepo(): { owner: string; repo: string } | null {
    const raw = (addOwner || addName).trim().replace(/^https?:\/\/github\.com\//, "");
    const parts = raw.split("/").filter(Boolean);
    if (parts.length >= 2) return { owner: parts[0], repo: parts[1] };
    if (addOwner.trim() && addName.trim()) {
      return { owner: addOwner.trim(), repo: addName.trim() };
    }
    return null;
  }

  async function addRepo(): Promise<void> {
    if (!detail || adding) return;
    const coords = parseRepo();
    if (!coords) {
      addError = "Enter owner/repository or fill both fields.";
      return;
    }
    adding = true;
    addError = null;
    const collectionId = detail.collection.id;
    const saved = await addRepoToCollection(collectionId, coords.owner, coords.repo);
    adding = false;
    if (!saved) return;
    addOwner = "";
    addName = "";
    await select(collectionId);
  }

  async function saveRepo(repo: CollectionRepo): Promise<void> {
    saving = repo.id;
    const saved = await updateCollectionRepo(repo, repo.status, repo.notes);
    if (saved && detail) {
      detail.repos = detail.repos.map((item) => (item.id === saved.id ? saved : item));
    }
    saving = null;
  }

  async function removeRepo(repo: CollectionRepo): Promise<void> {
    removing = repo.id;
    const removed = await removeCollectionRepo(repo);
    removing = null;
    if (removed && detail) {
      detail.repos = detail.repos.filter((item) => item.id !== repo.id);
      detail.collection.repoCount = Math.max(0, detail.collection.repoCount - 1);
      await loadCollections(true);
    }
  }

  function statusLabel(id: string): string {
    return STATUSES.find((status) => status.id === id)?.label ?? id;
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Collections</h1>
      <p class="view-sub">
        A local library of repositories you want to try, study, use or contribute to. GitHub
        metadata is read once; statuses and notes stay on this machine.
      </p>
    </div>
    <div class="head-actions">
      <button class="btn ghost" onclick={() => navigate("explore")}>
        <Icon name="explore" size={13} /> Explore
      </button>
      <button class="btn" onclick={() => (createOpen = !createOpen)}>
        <Icon name="plus" size={13} /> New collection
      </button>
    </div>
  </div>

  {#if createOpen}
    <div class="panel create-panel">
      <span class="mono-label">new local collection</span>
      <div class="create-row">
        <input class="field" bind:value={name} maxlength="80" placeholder="Collection name" aria-label="Collection name" />
        <input class="field grow" bind:value={description} maxlength="600" placeholder="What is this collection for? (optional)" aria-label="Collection description" />
        <button class="btn primary" onclick={() => void create()} disabled={creating || !name.trim()}>
          {creating ? "Creating…" : "Create"}
        </button>
        <button class="btn ghost" onclick={() => (createOpen = false)}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if library.error}
    <div class="panel notice">
      <p class="tone-danger">Collections could not be read.</p>
      <p class="faint">{library.error}</p>
      <button class="btn" onclick={() => void loadCollections(true)}>Retry</button>
    </div>
  {:else if library.collections.length === 0 && !library.loading}
    <div class="panel empty">
      <Icon name="collections" size={25} />
      <p class="empty-title">Start your own repository library</p>
      <p class="empty-copy">
        Create a collection such as “Wayland tools”, “Rust crates to study” or “Contributions”.
        Add repositories from Explore or by their GitHub name, then track a status and notes.
      </p>
      <button class="btn primary" onclick={() => (createOpen = true)}>Create the first collection</button>
    </div>
  {:else}
    <div class="library-layout">
      <aside class="panel collections">
        <div class="side-head">
          <span class="mono-label">collections</span>
          <span class="mono faint">{library.collections.length}</span>
        </div>
        <div class="collection-list">
          {#each library.collections as collection (collection.id)}
            <button
              class="collection"
              class:active={selectedId === collection.id}
              onclick={() => void select(collection.id)}
            >
              <span class="collection-name">{collection.name}</span>
              <span class="mono faint">{plural(collection.repoCount, "repo")}</span>
              {#if collection.description}<span class="collection-desc">{collection.description}</span>{/if}
            </button>
          {:else}
            <p class="side-empty">{library.loading ? "Reading collections…" : "No collections yet."}</p>
          {/each}
        </div>
      </aside>

      <div class="detail-column">
        {#if detailLoading}
          <p class="state mono">reading collection…</p>
        {:else if detail}
          <div class="panel collection-head">
            <div class="collection-title-row">
              <div>
                <h2 class="collection-title">{detail.collection.name}</h2>
                <p class="mono faint">
                  {plural(detail.collection.repoCount, "repository")} ·
                  updated {relativeTime(detail.collection.updatedAt)}
                </p>
              </div>
              <div class="row">
                {#if deleteConfirm}
                  <span class="mono-label">delete collection and notes?</span>
                  <button class="btn ghost danger" onclick={() => void removeCollection()}>Delete</button>
                  <button class="btn ghost" onclick={() => (deleteConfirm = false)}>Cancel</button>
                {:else}
                  <button class="btn ghost" onclick={() => (deleteConfirm = true)}>
                    <Icon name="trash" size={12} /> Delete
                  </button>
                {/if}
              </div>
            </div>
            {#if detail.collection.description}<p class="collection-about">{detail.collection.description}</p>{/if}
            <div class="add-row">
              <span class="mono-label">add by GitHub name</span>
              <input class="field" bind:value={addOwner} placeholder="owner" spellcheck="false" aria-label="Repository owner" />
              <span class="faint">/</span>
              <input class="field" bind:value={addName} placeholder="repository" spellcheck="false" aria-label="Repository name" />
              <button class="btn" onclick={() => void addRepo()} disabled={adding}>
                {adding ? "Reading GitHub…" : "Add repository"}
              </button>
              <span class="hint">or paste owner/repository</span>
            </div>
            {#if addError}<p class="error">{addError}</p>{/if}
          </div>

          {#if detail.repos.length === 0}
            <div class="panel empty small">
              <p class="empty-title">This collection is empty</p>
              <p class="empty-copy">Add a repository above or use Collect on an Explore result.</p>
            </div>
          {:else}
            <div class="repo-list">
              {#each detail.repos as repo (repo.id)}
                <article class="panel collection-repo">
                  <div class="repo-top">
                    <div class="repo-id">
                      <Icon name="folder-git" size={14} />
                      <button class="repo-link" onclick={() => void openRepoByCoords(repo.owner, repo.name)}>
                        {repo.fullName}
                      </button>
                      {#if repo.private}<span class="badge">private</span>{/if}
                      <span class="badge status {repo.status}">{statusLabel(repo.status)}</span>
                    </div>
                    <div class="repo-actions">
                      {#if removing === repo.id}
                        <span class="mono-label">remove?</span>
                        <button class="btn ghost" onclick={() => void removeRepo(repo)}>Yes</button>
                        <button class="btn ghost" onclick={() => (removing = null)}>No</button>
                      {:else}
                        <button class="btn ghost" onclick={() => void openRepoByCoords(repo.owner, repo.name)}>Browse</button>
                        <button class="btn ghost icon" title="Open on GitHub" aria-label={`Open ${repo.fullName} on GitHub`} onclick={() => open(repo.htmlUrl)}>
                          <Icon name="arrow-up-right" size={12} />
                        </button>
                        <button class="btn ghost icon" title="Remove from collection" aria-label={`Remove ${repo.fullName}`} onclick={() => (removing = repo.id)}>
                          <Icon name="trash" size={11} />
                        </button>
                      {/if}
                    </div>
                  </div>
                  {#if repo.description}<p class="repo-desc">{repo.description}</p>{/if}
                  <div class="repo-facts mono">
                    <span>{repo.language ?? "no language"}</span>
                    <span>★ {repo.stars}</span>
                    <span>Y {repo.forks}</span>
                    {#each repo.topics.slice(0, 5) as topic}<span class="topic">#{topic}</span>{/each}
                  </div>
                  <div class="repo-editor">
                    <label class="fLabel">
                      <span class="mono-label">status</span>
                      <select class="field" bind:value={repo.status}>
                        {#each STATUSES as status}<option value={status.id}>{status.label}</option>{/each}
                      </select>
                    </label>
                    <label class="fLabel grow">
                      <span class="mono-label">notes</span>
                      <textarea class="field notes" bind:value={repo.notes} maxlength="4000" placeholder="Why did you save this, what to try, what to change…"></textarea>
                    </label>
                    <button class="btn" onclick={() => void saveRepo(repo)} disabled={saving === repo.id}>
                      {saving === repo.id ? "Saving…" : "Save"}
                    </button>
                  </div>
                </article>
              {/each}
            </div>
          {/if}
        {:else}
          <div class="panel empty">
            <p class="empty-title">Select a collection</p>
            <p class="empty-copy">Choose a collection on the left to see repositories, statuses and notes.</p>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .head-actions {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }

  .create-panel {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    margin-bottom: var(--gap);
  }

  .create-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .create-row .grow {
    flex: 1;
  }

  .create-row .btn {
    flex: none;
    height: 24px;
    font-size: 9.5px;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 10px;
  }

  .library-layout {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: var(--gap);
    align-items: start;
  }

  .collections {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 170px);
    overflow: hidden;
  }

  .side-head {
    display: flex;
    justify-content: space-between;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
  }

  .collection-list {
    overflow-y: auto;
  }

  .collection {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .collection:hover {
    background: var(--bg-elev);
  }

  .collection.active {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 var(--accent);
  }

  .collection-name {
    font-size: 10.5px;
    font-weight: 600;
  }

  .collection-desc {
    font-size: 9px;
    color: var(--text-faint);
    line-height: 1.4;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .side-empty {
    padding: 12px 10px;
    font-size: 10px;
    color: var(--text-faint);
  }

  .detail-column {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .collection-head {
    padding: 10px;
  }

  .collection-title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .collection-title {
    font-size: var(--font-lg);
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .collection-about {
    margin-top: 4px;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-muted);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
  }

  .row .btn {
    height: 22px;
    font-size: 9px;
  }

  .danger {
    color: var(--danger);
  }

  .add-row {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 9px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .add-row .field {
    width: 100px;
    height: 23px;
    font-size: 9.5px;
  }

  .add-row .btn {
    height: 23px;
    font-size: 9.5px;
  }

  .repo-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .collection-repo {
    padding: 9px 10px;
  }

  .repo-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .repo-id {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-faint);
    min-width: 0;
  }

  .repo-link {
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--font-small);
    font-weight: 600;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-link:hover {
    color: var(--accent);
  }

  .repo-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }

  .repo-actions .btn {
    height: 22px;
    font-size: 9px;
    padding: 0 6px;
  }

  .badge.status.want_to_try {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
  }

  .badge.status.studying {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 35%, var(--border));
  }

  .badge.status.using {
    color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 35%, var(--border));
  }

  .badge.status.contributing {
    color: color-mix(in srgb, var(--accent) 70%, var(--text));
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
  }

  .badge.status.archived {
    color: var(--text-faint);
  }

  .repo-desc {
    margin-top: 3px;
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.45;
  }

  .repo-facts {
    display: flex;
    gap: 9px;
    flex-wrap: wrap;
    margin-top: 3px;
    font-size: 8.5px;
    color: var(--text-faint);
  }

  .topic {
    color: color-mix(in srgb, var(--accent) 75%, var(--text-faint));
  }

  .repo-editor {
    display: flex;
    align-items: flex-end;
    gap: 5px;
    margin-top: 7px;
    padding-top: 7px;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }

  .fLabel {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .fLabel .field {
    height: 23px;
    font-size: 9.5px;
  }

  .fLabel .notes {
    height: 40px;
    padding: 4px 6px;
    resize: vertical;
    font-size: 9.5px;
    line-height: 1.4;
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  .repo-editor .btn {
    height: 23px;
    font-size: 9px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 22px 18px;
    color: var(--text-faint);
  }

  .empty.small {
    padding: 14px 12px;
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

  .hint {
    font-size: 9px;
    color: var(--text-faint);
  }

  .state {
    padding: 4px 1px;
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  @media (max-width: 1050px) {
    .library-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .collections {
      max-height: 210px;
    }

    .repo-editor {
      flex-wrap: wrap;
    }
  }
</style>
