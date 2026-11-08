<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    type GithubRepo,
    type VisitedRepo,
    type WatchtowerEvent,
    type WatchtowerRepo,
  } from "../lib/api";
  import { relativeTime } from "../lib/format";
  import {
    github,
    isSignedIn,
    loadGithub,
    openRepo,
    openRepoByCoords,
  } from "../lib/state/github.svelte";
  import {
    WATCH_CATEGORIES,
    addWatchByFullName,
    loadWatchtower,
    loadWatchtowerDiscovery,
    markWatchtowerRead,
    pollWatchtower,
    removeWatch,
    setNativeAlerts,
    setWatchtowerEnabled,
    toggleWatchCategory,
    watchtower,
  } from "../lib/state/watchtower.svelte";
  import { navigate } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  let addInput = $state("");
  let addBusy = $state(false);
  let removeId = $state<number | null>(null);
  let eventFilter = $state<"all" | "unread">("all");
  let categoryFilter = $state("all");

  const signedIn = $derived(isSignedIn());
  const events = $derived(
    watchtower.events.filter(
      (event) =>
        (eventFilter === "all" || !event.seen) &&
        (categoryFilter === "all" || event.category === categoryFilter),
    ),
  );
  const lastChecked = $derived(
    watchtower.lastPollAt ? relativeTime(Math.floor(watchtower.lastPollAt / 1000)) : "never",
  );

  onMount(() => {
    void (async () => {
      if (!watchtower.loaded) await loadWatchtower();
      if (!github.status) await loadGithub();
      if (isSignedIn()) await loadWatchtowerDiscovery(true);
    })();
  });

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function open(url: string | null | undefined): void {
    if (!url) return;
    void api.openExternal(url).catch((cause) => message(cause));
  }

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (addBusy) return;
    addBusy = true;
    const ok = await addWatchByFullName(addInput);
    if (ok) addInput = "";
    addBusy = false;
  }

  async function confirmRemove(repo: WatchtowerRepo): Promise<void> {
    await removeWatch(repo);
    removeId = null;
  }

  function categoryLabel(id: string): string {
    return WATCH_CATEGORIES.find((category) => category.id === id)?.label ?? id;
  }

  function eventTime(event: WatchtowerEvent): string {
    const parsed = Date.parse(event.occurredAt);
    return relativeTime(Number.isFinite(parsed) ? Math.floor(parsed / 1000) : Math.floor(event.createdAt / 1000));
  }

  function browse(repo: WatchtowerRepo): void {
    void openRepoByCoords(repo.owner, repo.name);
  }

  function browseGithubRepo(repo: GithubRepo): void {
    openRepo(repo);
  }

  function browseVisited(repo: VisitedRepo): void {
    void openRepoByCoords(repo.owner, repo.name);
  }

  function updated(repo: GithubRepo): string {
    return relativeTime(Date.parse(repo.pushedAt ?? repo.updatedAt) / 1000);
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Watchtower</h1>
      <p class="view-sub">
        Favourite any repository and keep one local history of its commits, pull requests, issues,
        releases, reviews, workflow failures, stars and forks. Polling only reads from GitHub.
      </p>
    </div>
    <div class="head-actions">
      <button
        class="monitor-toggle"
        class:on={watchtower.enabled}
        onclick={() => setWatchtowerEnabled(!watchtower.enabled)}
      >
        <span class="dot" class:on={watchtower.enabled}></span>
        {watchtower.enabled ? "Monitoring on" : "Monitoring off"}
      </button>
      <button class="btn ghost" onclick={() => void pollWatchtower(true)} disabled={watchtower.polling || !signedIn}>
        <span class:spin={watchtower.polling}><Icon name="refresh" size={13} /></span>
        {watchtower.polling ? "Checking…" : "Check now"}
      </button>
    </div>
  </div>

  {#if !signedIn}
    <div class="panel notice">
      <div>
        <span class="badge warn">GitHub required</span>
        <p>Watchtower polls the GitHub API. Connect an account to add favourites and import events.</p>
      </div>
      <button class="btn" onclick={() => navigate("repositories")}>Connect GitHub</button>
    </div>
  {:else if watchtower.error}
    <div class="panel notice">
      <div>
        <span class="badge warn">local state unavailable</span>
        <p>{watchtower.error}</p>
      </div>
      <button class="btn" onclick={() => void loadWatchtower(true)}>Retry</button>
    </div>
  {/if}

  <div class="summary">
    <div class="panel stat">
      <span class="mono-label">favourited</span>
      <strong class="mono">{watchtower.repos.length}</strong>
      <span class="hint">watched repositories</span>
    </div>
    <div class="panel stat">
      <span class="mono-label">starred</span>
      <strong class="mono">{watchtower.discoveryLoading ? "—" : watchtower.starred.length}</strong>
      <span class="hint">on GitHub</span>
    </div>
    <div class="panel stat">
      <span class="mono-label">visited</span>
      <strong class="mono">{watchtower.visited.length}</strong>
      <span class="hint">local history</span>
    </div>
    <div class="panel stat">
      <span class="mono-label">unread</span>
      <strong class="mono" class:accent={watchtower.unread > 0}>{watchtower.unread}</strong>
      <span class="hint">alerts in history</span>
    </div>
    <div class="panel stat">
      <span class="mono-label">last check</span>
      <strong class="mono small">{lastChecked}</strong>
      <span class="hint">{watchtower.lastPollNotModified} × HTTP 304</span>
    </div>
    <div class="panel stat">
      <span class="mono-label">background</span>
      <strong class="mono small">{watchtower.enabled ? "enabled" : "off"}</strong>
      <span class="hint">3-minute throttle · opt-in</span>
    </div>
  </div>

  <div class="panel radar">
    <div class="panel-head radar-head">
      <div>
        <span class="mono-label">repository radar</span>
        <p class="hint">
          Stars stay on GitHub. Favourites and visit history stay local. Recommendations are
          derived from the languages and topics in those sources.
        </p>
      </div>
      <button
        class="btn ghost"
        onclick={() => void loadWatchtowerDiscovery(true)}
        disabled={watchtower.discoveryLoading || !signedIn}
      >
        <span class:spin={watchtower.discoveryLoading}><Icon name="refresh" size={12} /></span>
        {watchtower.discoveryLoading ? "Reading…" : "Refresh radar"}
      </button>
    </div>

    {#if watchtower.discoveryError}
      <p class="radar-error">{watchtower.discoveryError}</p>
    {/if}

    <div class="radar-grid">
      <section class="radar-section">
        <div class="radar-title">
          <span class="mono-label">starred on GitHub</span>
          <span class="mono faint">{watchtower.starred.length}</span>
        </div>
        <div class="radar-list">
          {#each watchtower.starred as repo (repo.id)}
            <div class="radar-item">
              <button class="radar-main" onclick={() => browseGithubRepo(repo)} title={`Open ${repo.fullName} in GRAFT`}>
                <span class="radar-name">{repo.fullName}</span>
                {#if repo.description}<span class="radar-desc">{repo.description}</span>{/if}
                <span class="radar-meta mono">
                  <span>{repo.language ?? "no language"}</span>
                  <span>★ {repo.stargazersCount}</span>
                  <span>updated {updated(repo)}</span>
                </span>
              </button>
              <button class="radar-open" title="Open on GitHub" aria-label={`Open ${repo.fullName} on GitHub`} onclick={() => open(repo.htmlUrl)}>
                <Icon name="arrow-up-right" size={11} />
              </button>
            </div>
          {:else}
            <p class="radar-empty">
              {watchtower.discoveryLoading ? "Reading your GitHub stars…" : "No starred repositories returned."}
            </p>
          {/each}
        </div>
      </section>

      <section class="radar-section">
        <div class="radar-title">
          <span class="mono-label">visited often</span>
          <span class="mono faint">{watchtower.visited.length}</span>
        </div>
        <div class="radar-list">
          {#each watchtower.visited as repo (repo.fullName)}
            <div class="radar-item">
              <button class="radar-main" onclick={() => browseVisited(repo)} title={`Open ${repo.fullName} in GRAFT`}>
                <span class="radar-name">{repo.fullName}</span>
                {#if repo.description}<span class="radar-desc">{repo.description}</span>{/if}
                <span class="radar-meta mono">
                  <span class="visits">{repo.visitCount} visit{repo.visitCount === 1 ? "" : "s"}</span>
                  <span>{repo.language ?? "no language"}</span>
                  <span>{relativeTime(repo.lastVisitedAt)}</span>
                </span>
              </button>
              <button class="radar-open" title="Open on GitHub" aria-label={`Open ${repo.fullName} on GitHub`} onclick={() => open(repo.htmlUrl)}>
                <Icon name="arrow-up-right" size={11} />
              </button>
            </div>
          {:else}
            <p class="radar-empty">
              Open repositories in GRAFT. Visit counts are stored locally and appear here.
            </p>
          {/each}
        </div>
      </section>

      <section class="radar-section">
        <div class="radar-title">
          <span class="mono-label">recommended similar</span>
          <span class="mono faint">{watchtower.recommended.length}</span>
        </div>
        <div class="radar-list">
          {#each watchtower.recommended as item (item.repo.id)}
            <div class="radar-item">
              <button class="radar-main" onclick={() => browseGithubRepo(item.repo)} title={`Open ${item.repo.fullName} in GRAFT`}>
                <span class="radar-name">{item.repo.fullName}</span>
                <span class="radar-reason">{item.reason}</span>
                <span class="radar-meta mono">
                  <span>{item.repo.language ?? "no language"}</span>
                  <span>★ {item.repo.stargazersCount}</span>
                  <span>updated {updated(item.repo)}</span>
                </span>
              </button>
              <button class="radar-open" title="Open on GitHub" aria-label={`Open ${item.repo.fullName} on GitHub`} onclick={() => open(item.repo.htmlUrl)}>
                <Icon name="arrow-up-right" size={11} />
              </button>
            </div>
          {:else}
            <p class="radar-empty">
              {watchtower.discoveryLoading
                ? "Deriving similar repositories…"
                : "Star, favourite or visit repositories to build recommendation signals."}
            </p>
          {/each}
        </div>
      </section>
    </div>
  </div>

  <div class="panel categories">
    <div class="categories-head">
      <span class="mono-label">alert categories</span>
      <span class="hint">Changes apply to the next check; existing history is kept.</span>
    </div>
    <div class="chips">
      {#each WATCH_CATEGORIES as category}
        <button
          class="chip"
          class:active={watchtower.categories.includes(category.id)}
          aria-pressed={watchtower.categories.includes(category.id)}
          onclick={() => toggleWatchCategory(category.id)}
        >
          {category.label}
        </button>
      {/each}
      <span class="sep"></span>
      <label class="native">
        <input
          type="checkbox"
          checked={watchtower.native}
          onchange={(event) => setNativeAlerts(event.currentTarget.checked)}
        />
        Desktop notifications
      </label>
    </div>
  </div>

  <div class="layout">
    <aside class="panel watch-list">
      <div class="panel-head">
        <div>
          <span class="mono-label">favourites</span>
          <p class="hint">Add any public or private repository you can read.</p>
        </div>
      </div>
      <form class="add" onsubmit={add}>
        <input
          class="field mono"
          bind:value={addInput}
          placeholder="owner/repository"
          spellcheck="false"
          autocomplete="off"
          aria-label="Repository to watch"
        />
        <button class="btn" type="submit" disabled={addBusy || !addInput.trim() || !signedIn}>
          {addBusy ? "Adding…" : "Watch"}
        </button>
      </form>

      <div class="repo-list">
        {#each watchtower.repos as repo (repo.id)}
          <div class="repo">
            <button class="repo-main" onclick={() => browse(repo)} title={`Open ${repo.fullName} in GRAFT`}>
              <span class="repo-name">
                <Icon name="folder-git" size={13} />
                {repo.name}
              </span>
              <span class="repo-owner mono">{repo.owner}</span>
              <span class="repo-facts mono">
                {repo.language ?? "no language"}
                {#if repo.private}<span class="badge">private</span>{/if}
                <span>checked {repo.lastPolledAt ? relativeTime(repo.lastPolledAt) : "never"}</span>
              </span>
              {#if repo.lastError}<span class="repo-error">{repo.lastError}</span>{/if}
            </button>
            <div class="repo-actions">
              <button class="btn ghost icon" title="Open on GitHub" aria-label={`Open ${repo.fullName} on GitHub`} onclick={() => open(repo.htmlUrl)}>
                <Icon name="arrow-up-right" size={12} />
              </button>
              {#if removeId === repo.id}
                <span class="confirm mono-label">remove?</span>
                <button class="btn ghost danger-text" onclick={() => void confirmRemove(repo)}>Yes</button>
                <button class="btn ghost" onclick={() => (removeId = null)}>No</button>
              {:else}
                <button class="btn ghost icon" title="Stop watching" aria-label={`Stop watching ${repo.fullName}`} onclick={() => (removeId = repo.id)}>
                  <Icon name="trash" size={12} />
                </button>
              {/if}
            </div>
          </div>
        {:else}
          <div class="empty">
            <Icon name="radar" size={21} />
            <p>No watched repositories yet.</p>
            <p class="hint">Add a repository above, or use Watch in its GRAFT workspace.</p>
          </div>
        {/each}
      </div>
    </aside>

    <section class="panel feed">
      <div class="panel-head feed-head">
        <div>
          <span class="mono-label">alert history</span>
          <p class="hint">
            Stored in SQLite. The first check for a new favourite records a baseline instead of replaying old activity.
          </p>
        </div>
        <div class="feed-actions">
          <div class="segmented">
            <button class="seg" class:active={eventFilter === "all"} onclick={() => (eventFilter = "all")}>All</button>
            <button class="seg" class:active={eventFilter === "unread"} onclick={() => (eventFilter = "unread")}>Unread</button>
          </div>
          <select class="field filter" bind:value={categoryFilter} aria-label="Filter alert category">
            <option value="all">All categories</option>
            {#each WATCH_CATEGORIES as category}
              <option value={category.id}>{category.label}</option>
            {/each}
          </select>
          <button class="btn ghost" onclick={() => void markWatchtowerRead(null)} disabled={watchtower.unread === 0}>
            Mark all read
          </button>
        </div>
      </div>

      {#if events.length === 0}
        <div class="empty feed-empty">
          <Icon name="bell" size={21} />
          <p>{watchtower.events.length === 0 ? "No imported activity yet." : "No alerts match this filter."}</p>
          <p class="hint">
            {#if watchtower.repos.length === 0}
              Watch a repository first. Nothing is fabricated when there is no history.
            {:else if watchtower.enabled}
              Monitoring is on. GRAFT revalidates each favourite on the next sync.
            {:else}
              Monitoring is off. Use Check now or enable background monitoring.
            {/if}
          </p>
        </div>
      {:else}
        <ul class="events">
          {#each events as event (event.id)}
            <li class="event" class:unread={!event.seen}>
              <button class="event-body" onclick={() => void markWatchtowerRead(event.id)} title="Mark this alert read">
                <span class="event-head">
                  <span class="category {event.category}">{categoryLabel(event.category)}</span>
                  <span class="event-repo mono">{event.repoFullName}</span>
                  <span class="event-time mono">{eventTime(event)}</span>
                  {#if !event.seen}<span class="unread-dot" aria-label="Unread"></span>{/if}
                </span>
                <span class="event-title">{event.title}</span>
                <span class="event-detail">{event.detail}</span>
              </button>
              <button class="btn ghost icon event-open" title="Open on GitHub" aria-label="Open alert on GitHub" onclick={() => open(event.url)}>
                <Icon name="arrow-up-right" size={12} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>

  <p class="footnote">
    Watchtower is read-only. It stores favourites, ETags and observed events locally, never stars,
    watches or reads anything on GitHub on your behalf, and never sends a notification for the first
    baseline check. Background polling starts only after you enable it.
  </p>
</section>

<style>
  .head-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .monitor-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-muted);
    font-size: 10.5px;
    cursor: pointer;
  }

  .monitor-toggle.on {
    border-color: color-mix(in srgb, var(--ok) 55%, var(--border));
    color: var(--ok);
    background: color-mix(in srgb, var(--ok) 6%, var(--surface));
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .dot.on {
    background: var(--ok);
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
    margin-top: 3px;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .summary {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 6px;
    margin-bottom: var(--gap);
  }

  .stat {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: end;
    gap: 2px 10px;
    padding: 8px 10px 9px;
  }

  .stat strong {
    font-size: 23px;
    line-height: 1;
    font-weight: 600;
    color: var(--text);
    grid-row: span 2;
    text-align: right;
  }

  .stat strong.small {
    font-size: 12px;
    line-height: 1.4;
  }

  .stat strong.accent {
    color: var(--accent);
  }

  .stat .hint {
    font-size: 9.5px;
  }

  .radar {
    margin-bottom: var(--gap);
    overflow: hidden;
  }

  .radar-head {
    align-items: center;
  }

  .radar-head .btn {
    height: 23px;
    font-size: 9.5px;
    flex: none;
  }

  .radar-error {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    color: var(--warn);
    font-size: 9.5px;
    line-height: 1.45;
  }

  .radar-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .radar-section {
    min-width: 0;
    border-right: 1px solid var(--border);
  }

  .radar-section:last-child {
    border-right: 0;
  }

  .radar-title {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 6px 9px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
    background: var(--bg-elev);
  }

  .radar-list {
    max-height: 230px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .radar-item {
    display: flex;
    align-items: flex-start;
    gap: 2px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .radar-item:hover {
    background: var(--bg-elev);
  }

  .radar-main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
    padding: 6px 4px 6px 9px;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .radar-name {
    font-size: 10.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .radar-desc {
    font-size: 9px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .radar-reason {
    font-size: 8.5px;
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .radar-meta {
    display: flex;
    gap: 7px;
    min-width: 0;
    overflow: hidden;
    font-size: 8px;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .radar-meta .visits {
    color: var(--accent);
  }

  .radar-open {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 23px;
    height: 23px;
    margin: 5px 4px 0 0;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-faint);
    cursor: pointer;
    flex: none;
  }

  .radar-open:hover {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .radar-empty {
    padding: 12px 10px;
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .categories {
    padding: 8px 10px 9px;
    margin-bottom: var(--gap);
  }

  .categories-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 6px;
  }

  .categories-head .hint {
    margin: 0;
  }

  .chips {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text-faint);
    font-size: 9.5px;
    cursor: pointer;
  }

  .chip.active {
    border-color: color-mix(in srgb, var(--accent) 65%, var(--border));
    background: var(--accent-soft);
    color: var(--accent);
  }

  .sep {
    width: 1px;
    height: 15px;
    background: var(--border);
    margin: 0 5px;
  }

  .native {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    color: var(--text-muted);
    cursor: pointer;
  }

  .native input {
    accent-color: var(--accent);
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(250px, 0.78fr) minmax(0, 1.55fr);
    gap: var(--gap);
    align-items: stretch;
    min-height: 430px;
  }

  .watch-list,
  .feed {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .panel-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
    padding: 9px 11px;
    border-bottom: 1px solid var(--border);
  }

  .panel-head .hint {
    margin-top: 2px;
    max-width: 62ch;
  }

  .add {
    display: flex;
    gap: 5px;
    padding: 8px 9px;
    border-bottom: 1px solid var(--border);
  }

  .add .field {
    flex: 1;
    min-width: 0;
    height: 24px;
    font-size: 10px;
  }

  .add .btn {
    height: 24px;
    font-size: 10px;
  }

  .repo-list {
    flex: 1;
    overflow-y: auto;
  }

  .repo {
    display: flex;
    align-items: flex-start;
    gap: 5px;
    padding: 8px 7px 8px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .repo:hover {
    background: var(--bg-elev);
  }

  .repo-main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    font: inherit;
    cursor: pointer;
  }

  .repo-name {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-owner,
  .repo-facts {
    font-size: 9px;
    color: var(--text-faint);
  }

  .repo-facts {
    display: flex;
    align-items: center;
    gap: 7px;
    white-space: nowrap;
    overflow: hidden;
  }

  .repo-error {
    margin-top: 2px;
    font-size: 9px;
    line-height: 1.4;
    color: var(--danger);
  }

  .repo-actions {
    display: flex;
    align-items: center;
    gap: 1px;
    flex: none;
  }

  .repo-actions .btn {
    height: 21px;
    padding: 0 5px;
    font-size: 9px;
  }

  .confirm {
    font-size: 8px;
  }

  .danger-text {
    color: var(--danger);
  }

  .feed-head {
    align-items: center;
    flex-wrap: wrap;
  }

  .feed-actions {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }

  .segmented {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .seg {
    height: 21px;
    padding: 0 7px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .seg:last-child {
    border-right: 0;
  }

  .seg.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .filter {
    height: 21px;
    font-size: 9px;
    padding: 0 5px;
    max-width: 150px;
  }

  .feed-actions .btn {
    height: 21px;
    font-size: 9px;
    padding: 0 6px;
  }

  .events {
    flex: 1;
    overflow-y: auto;
  }

  .event {
    display: flex;
    align-items: stretch;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
  }

  .event:hover {
    background: var(--bg-elev);
  }

  .event.unread {
    background: color-mix(in srgb, var(--accent) 4%, transparent);
  }

  .event-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
    padding: 7px 9px 7px 11px;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    font: inherit;
    cursor: pointer;
  }

  .event-head {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .category {
    padding: 0 5px;
    border: 1px solid var(--border);
    border-radius: 2px;
    color: var(--text-muted);
    font-size: 8px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .category.workflows {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 45%, var(--border));
  }

  .category.pull_requests,
  .category.reviews {
    color: var(--info);
  }

  .category.releases {
    color: var(--warn);
  }

  .event-repo {
    font-size: 9px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .event-time {
    margin-left: auto;
    font-size: 8.5px;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .event-title {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .event-detail {
    font-size: 9.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unread-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
  }

  .event-open {
    align-self: center;
    margin-right: 4px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 20px 14px;
    color: var(--text-faint);
  }

  .empty p {
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .empty-title {
    font-size: var(--font-small);
    font-weight: 600;
    color: var(--text);
  }

  .empty .hint,
  .hint {
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .footnote {
    margin-top: 9px;
    font-size: 9.5px;
    line-height: 1.55;
    color: var(--text-faint);
    max-width: 100ch;
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

  @media (max-width: 1180px) {
    .summary {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .radar-grid {
      grid-template-columns: minmax(0, 1fr);
    }

    .radar-section {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .radar-section:last-child {
      border-bottom: 0;
    }

    .radar-list {
      max-height: 190px;
    }
  }

  @media (max-width: 1020px) {
    .summary {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .watch-list {
      max-height: 310px;
    }
  }
</style>
