<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api } from "../lib/api";
  import { plural, relativeTime } from "../lib/format";
  import {
    github,
    githubUser,
    loadGithub,
    loadRepos,
    openRepo,
    searchRepos,
    setClientId,
    setRepoScope,
    signOut,
    visibleRepos,
  } from "../lib/state/github.svelte";
  import { notify } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  const user = $derived(githubUser());
  const signedIn = $derived(github.status?.signedIn ?? false);
  const repos = $derived(visibleRepos());
  const repoScope = $derived(github.scope);
  const displayRepos = $derived(repoScope === "mine" ? repos : github.discover.results);
  const savedClientId = $derived(github.status?.clientId ?? "");

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

  let clientIdInput = $state("");
  let clientIdTouched = $state(false);
  let clientIdSaving = $state(false);

  let device = $state<{
    code: string;
    deviceCode: string;
    expiresAt: number;
    interval: number;
  } | null>(null);
  let deviceBusy = $state(false);
  let deviceError = $state<string | null>(null);
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let cancelled = true;

  let tokenInput = $state("");
  let tokenBusy = $state(false);
  let tokenError = $state<string | null>(null);
  let showToken = $state(false);

  type RepoView = "list" | "cards" | "compact";
  let repoView = $state<RepoView>("list");

  function setRepoView(next: RepoView): void {
    repoView = next;
    void api.setSetting("repos.view", next).catch(() => undefined);
  }

  function submitSearch(event: SubmitEvent): void {
    event.preventDefault();
    void searchRepos(true);
  }

  function pickTopic(topic: string): void {
    github.discover.topic = github.discover.topic === topic ? "" : topic;
    void searchRepos(true);
  }

  function clearSearch(): void {
    github.discover.query = "";
    github.discover.language = "";
    github.discover.topic = "";
    github.discover.author = "";
    github.discover.sort = "best";
    github.discover.results = [];
    github.discover.totalCount = 0;
    github.discover.page = 1;
    github.discover.hasMore = false;
    github.discover.error = null;
    github.discover.searched = false;
  }

  function refreshCurrent(): void {
    if (repoScope === "mine") void loadRepos(true);
    else void searchRepos(true);
  }

  $effect(() => {
    if (!clientIdTouched) clientIdInput = savedClientId;
  });

  onMount(() => {
    void (async () => {
      const stored = await api.getSetting("repos.view").catch(() => null);
      if (stored === "cards" || stored === "compact" || stored === "list") repoView = stored;
      const storedScope = await api.getSetting("repos.scope").catch(() => null);
      if (storedScope === "mine" || storedScope === "explore") github.scope = storedScope;
      if (!github.status) await loadGithub();
      if (github.status?.signedIn && github.repos.length === 0) await loadRepos(true);
    })();
  });

  onDestroy(() => {
    cancelled = true;
    if (pollTimer) clearTimeout(pollTimer);
  });

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function open(url: string | null | undefined): void {
    if (!url) return;
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

  async function saveClientId(): Promise<void> {
    clientIdSaving = true;
    const saved = await setClientId(clientIdInput);
    clientIdSaving = false;
    if (saved) {
      clientIdTouched = false;
      notify("Client ID saved", "ok");
    }
  }

  async function startDevice(): Promise<void> {
    deviceError = null;
    deviceBusy = true;
    try {
      const start = await api.startDeviceFlow();
      device = {
        code: start.userCode,
        deviceCode: start.deviceCode,
        expiresAt: Date.now() + start.expiresIn * 1000,
        interval: Math.max(5, start.interval),
      };
      cancelled = false;
      open(start.verificationUri);
      if (device) schedule(device.interval);
    } catch (cause) {
      deviceError = message(cause);
    } finally {
      deviceBusy = false;
    }
  }

  function schedule(seconds: number): void {
    if (pollTimer) clearTimeout(pollTimer);
    pollTimer = setTimeout(() => void poll(), Math.max(5, seconds) * 1000);
  }

  async function poll(): Promise<void> {
    const current = device;
    if (!current || cancelled) return;
    if (Date.now() > current.expiresAt) {
      deviceError = "The device code expired. Start the sign-in again.";
      device = null;
      return;
    }

    try {
      const result = await api.pollDeviceFlow(current.deviceCode);
      if (result.status === "pending") {
        schedule(current.interval);
      } else if (result.status === "slowDown") {
        current.interval = result.interval + 5;
        schedule(current.interval);
      } else if (result.status === "failed") {
        deviceError = result.message;
        device = null;
      } else {
        device = null;
        cancelled = true;
        if (pollTimer) clearTimeout(pollTimer);
        notify(`Signed in as ${result.user.login}`, "ok");
        await loadGithub();
        await loadRepos(true);
      }
    } catch (cause) {
      deviceError = message(cause);
      device = null;
    }
  }

  function cancelDevice(): void {
    cancelled = true;
    if (pollTimer) clearTimeout(pollTimer);
    device = null;
    deviceError = null;
  }

  async function copyCode(): Promise<void> {
    if (!device) return;
    try {
      if (!navigator.clipboard) throw new Error("clipboard unavailable");
      await navigator.clipboard.writeText(device.code);
      notify("Code copied", "ok");
    } catch {
      notify("Clipboard unavailable — select the code manually", "danger");
    }
  }

  async function submitToken(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    tokenError = null;
    tokenBusy = true;
    try {
      const profile = await api.saveGithubToken(tokenInput);
      tokenInput = "";
      showToken = false;
      notify(`Signed in as ${profile.login}`, "ok");
      await loadGithub();
      await loadRepos(true);
    } catch (cause) {
      tokenError = message(cause);
    } finally {
      tokenBusy = false;
    }
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Repositories</h1>
      <p class="view-sub">
        Repositories are read from the GitHub REST API by the Rust engine. Your token stays in the
        system keyring — it is never written to SQLite, a log or this view.
      </p>
    </div>
    {#if signedIn}
      <span class="badge ok">authenticated</span>
    {/if}
  </div>

  {#if github.loading && !github.status}
    <p class="state mono">checking stored credentials…</p>
  {:else if !signedIn}
    {#if github.error}
      <div class="panel notice">
        <p class="tone-danger">The GitHub status could not be read.</p>
        <p class="faint">{github.error}</p>
        <button class="btn" onclick={() => loadGithub()}>Retry</button>
      </div>
    {/if}

    <div class="auth-grid">
      <div class="panel auth">
        <div class="auth-head">
          <span class="step mono">01</span>
          <div>
            <h2>OAuth app</h2>
            <p>GRAFT uses the device flow, so there is no client secret to store.</p>
          </div>
        </div>

        <div class="field-row">
          <span class="mono-label">Client ID</span>
          <div class="row">
            <input
              class="field mono"
              bind:value={clientIdInput}
              oninput={() => (clientIdTouched = true)}
              placeholder="20 characters"
              spellcheck="false"
              autocomplete="off"
            />
            <button
              class="btn"
              onclick={saveClientId}
              disabled={clientIdSaving || clientIdInput.trim() === savedClientId}
            >
              {clientIdSaving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>

        <p class="hint">
          Create an OAuth app in GitHub → Settings → Developer settings, tick “Enable Device Flow”,
          then paste the client ID here.
          <button class="link" onclick={() => open("https://github.com/settings/developers")}>
            Open GitHub settings ↗
          </button>
        </p>
        {#if savedClientId}
          <p class="hint mono">saved · {savedClientId}</p>
        {/if}
      </div>

      <div class="panel auth">
        <div class="auth-head">
          <span class="step mono">02</span>
          <div>
            <h2>Authorise GRAFT</h2>
            <p>GitHub shows a short code; GRAFT waits for your approval.</p>
          </div>
        </div>

        {#if device}
          <div class="code-block">
            <span class="mono-label">enter this code on github.com</span>
            <button class="code mono" onclick={copyCode} title="Copy code">{device.code}</button>
            <span class="waiting mono-label">
              <span class="pulse"></span> waiting for approval…
            </span>
          </div>
          <div class="row">
            <button class="btn" onclick={() => open("https://github.com/login/device")}>
              Open github.com/login/device
            </button>
            <button class="btn ghost" onclick={cancelDevice}>Cancel</button>
          </div>
        {:else}
          <button
            class="btn primary wide"
            onclick={startDevice}
            disabled={deviceBusy || !savedClientId}
          >
            <Icon name="github" size={14} />
            {deviceBusy ? "Requesting code…" : "Connect GitHub"}
          </button>
          {#if !savedClientId}
            <p class="hint">Save a client ID first — the device flow needs one.</p>
          {/if}
        {/if}

        {#if deviceError}<p class="error">{deviceError}</p>{/if}
        {#if github.status?.keyringError}<p class="error">{github.status.keyringError}</p>{/if}
        {#if github.status?.tokenError}<p class="error">{github.status.tokenError}</p>{/if}
      </div>
    </div>

    <div class="panel token">
      <button
        class="token-head"
        onclick={() => (showToken = !showToken)}
        aria-expanded={showToken}
      >
        <span class="mono-label">use a personal access token instead</span>
        <Icon name={showToken ? "chevron-up" : "chevron-down"} size={12} />
      </button>
      {#if showToken}
        <form class="token-body" onsubmit={submitToken}>
          <input
            class="field mono"
            type="password"
            bind:value={tokenInput}
            placeholder="ghp_… or github_pat_…"
            autocomplete="off"
            spellcheck="false"
          />
          <button class="btn" type="submit" disabled={tokenBusy || !tokenInput.trim()}>
            {tokenBusy ? "Checking…" : "Store token"}
          </button>
        </form>
        <p class="hint">
          The token is verified against api.github.com and only then written to the Secret Service.
          Use <span class="mono">repo</span>, <span class="mono">read:user</span> and
          <span class="mono">user</span> scopes to match the device flow. The
          <span class="mono">user</span> scope is needed to set your GitHub status.
        </p>
        {#if tokenError}<p class="error">{tokenError}</p>{/if}
      {/if}
    </div>
  {:else}
    <div class="account">
      <div class="identity">
        {#if user}
          <img class="avatar" src={user.avatarUrl} alt="" referrerpolicy="no-referrer" />
          <div class="who">
            <span class="login">{user.login}</span>
            <span class="mono-label">
              {user.name ?? "github account"}
              {#if github.status?.scopes}· {github.status.scopes}{/if}
            </span>
          </div>
        {:else}
          <div class="who">
            <span class="login">{github.status?.cachedUser?.login ?? "GitHub"}</span>
            <span class="mono-label">profile unavailable offline</span>
          </div>
        {/if}
      </div>
      <div class="row">
        <button class="btn ghost" onclick={() => open(user?.htmlUrl)} disabled={!user}>
          Profile ↗
        </button>
        <button class="btn ghost" onclick={() => signOut()}>Sign out</button>
      </div>
    </div>

    {#if github.status?.tokenError}
      <div class="panel notice">
        <p class="tone-danger">GitHub rejected the stored token.</p>
        <p class="faint">{github.status.tokenError}</p>
        <button class="btn" onclick={() => signOut()}>Clear token and sign in again</button>
      </div>
    {/if}

    <div class="toolbar panel">
      <div class="scope-switch" role="group" aria-label="Repository source">
        <button class="scope-btn" class:active={repoScope === "mine"} onclick={() => setRepoScope("mine")}>Mine</button>
        <button class="scope-btn" class:active={repoScope === "explore"} onclick={() => setRepoScope("explore")}>Explore</button>
      </div>
      {#if repoScope === "mine"}
        <Icon name="search" size={13} />
        <input
          class="search"
          bind:value={github.filter}
          placeholder="Filter by name, language or visibility…"
          spellcheck="false"
          aria-label="Filter repositories"
        />
        <span class="mono-label">
          {plural(github.repos.length, "repository", "repositories")}
        </span>
      {:else}
        <form class="discover-form" onsubmit={submitSearch}>
          <Icon name="search" size={13} />
          <input
            class="search"
            bind:value={github.discover.query}
            placeholder="Search GitHub — hyprland, ricing, linux…"
            spellcheck="false"
            aria-label="Search public repositories"
          />
          <button class="btn" type="submit" disabled={github.discover.loading}>
            {github.discover.loading ? "Searching…" : "Search"}
          </button>
        </form>
        <span class="mono-label">
          {#if github.discover.searched}
            {plural(github.discover.totalCount, "result")}
          {:else}
            public GitHub
          {/if}
        </span>
      {/if}
      <div class="view-switch" role="group" aria-label="Repository layout">
        {#each [["list", "List"], ["cards", "Cards"], ["compact", "Compact"]] as [id, label]}
          <button
            class="view-btn"
            class:active={repoView === id}
            title={`${label} view`}
            aria-pressed={repoView === id}
            onclick={() => setRepoView(id as RepoView)}
          >{label}</button>
        {/each}
      </div>
      <button
        class="btn ghost icon"
        title="Refresh from GitHub"
        aria-label="Refresh repositories"
        onclick={refreshCurrent}
        disabled={repoScope === "mine" ? github.reposLoading : github.discover.loading}
      >
        <span class:spin={repoScope === "mine" ? github.reposLoading : github.discover.loading}><Icon name="refresh" size={13} /></span>
      </button>
    </div>

    {#if repoScope === "explore"}
      <div class="panel discover">
        <div class="discover-grid">
          <label class="field-label">
            <span class="mono faint">language</span>
            <input class="field" bind:value={github.discover.language} placeholder="Rust, QML, TypeScript" list="discover-languages" />
          </label>
          <datalist id="discover-languages">
            {#each ["Rust", "TypeScript", "Python", "QML", "C++", "Go", "Shell", "Lua", "Nix"] as language}
              <option value={language}></option>
            {/each}
          </datalist>
          <label class="field-label">
            <span class="mono faint">topic</span>
            <input class="field" bind:value={github.discover.topic} placeholder="hyprland, ricing…" spellcheck="false" />
          </label>
          <label class="field-label">
            <span class="mono faint">author</span>
            <input class="field" bind:value={github.discover.author} placeholder="login or @login" spellcheck="false" />
          </label>
          <label class="field-label">
            <span class="mono faint">sort</span>
            <select class="field" bind:value={github.discover.sort} onchange={() => github.discover.searched && void searchRepos(true)}>
              <option value="best">Best match</option>
              <option value="stars">Most stars</option>
              <option value="forks">Most forks</option>
              <option value="updated">Recently updated</option>
            </select>
          </label>
        </div>
        <div class="discover-topics">
          <span class="mono-label">topics</span>
          {#each TOPICS as topic}
            <button
              class="topic-chip"
              class:active={github.discover.topic === topic.id}
              onclick={() => pickTopic(topic.id)}
            >{topic.label}</button>
          {/each}
        </div>
        <div class="discover-foot">
          <span class="hint">Live GitHub search. Filters become <span class="mono">language:</span>, <span class="mono">topic:</span> and <span class="mono">user:</span> qualifiers.</span>
          <button class="link" onclick={clearSearch}>Reset filters</button>
        </div>
      </div>
    {/if}

    {#if repoScope === "mine"}
      {#if github.reposLoading && github.repos.length === 0}
        <p class="state mono">loading repositories from api.github.com…</p>
      {:else if github.reposError}
        <div class="panel notice">
          <p class="tone-danger">Repositories could not be loaded.</p>
          <p class="faint">{github.reposError}</p>
          <button class="btn" onclick={() => loadRepos(true)}>Retry</button>
        </div>
      {:else if github.repos.length === 0}
        <div class="panel empty">
          <Icon name="folder-git" size={22} />
          <p class="empty-title">This account has no repositories GRAFT can see</p>
          <p class="empty-copy">
            The listing includes repositories you own, collaborate on or reach through an organisation.
            Re-authorise with the <span class="mono">repo</span> scope if private repositories are
            missing.
          </p>
          <button class="btn" onclick={() => loadRepos(true)}>Check again</button>
        </div>
      {:else if repos.length === 0}
        <p class="state mono">No repository matches “{github.filter}”.</p>
      {/if}
    {:else}
      {#if github.discover.error}
        <div class="panel notice">
          <p class="tone-danger">GitHub search failed.</p>
          <p class="faint">{github.discover.error}</p>
          <button class="btn" onclick={() => searchRepos(true)}>Retry</button>
        </div>
      {:else if github.discover.loading && github.discover.results.length === 0}
        <p class="state mono">searching public repositories on api.github.com…</p>
      {:else if !github.discover.searched}
        <div class="panel empty">
          <Icon name="search" size={22} />
          <p class="empty-title">Explore GitHub without leaving GRAFT</p>
          <p class="empty-copy">
            Search all public repositories by keyword, language, topic or author. Try
            <span class="mono">hyprland</span>, <span class="mono">ricing</span> or
            <span class="mono">linux</span>, or pick a topic above.
          </p>
        </div>
      {:else if github.discover.results.length === 0}
        <p class="state mono">No public repository matched this search.</p>
      {/if}
    {/if}

    {#if displayRepos.length > 0}
      <ul class="repos {repoView}">
        {#each displayRepos as repo (repo.id)}
          <li class="panel repo">
            <div class="repo-main">
              <div class="repo-title">
                <Icon name="folder-git" size={14} />
                <span class="repo-name">{repo.name}</span>
                <span class="owner mono">{repo.owner?.login ?? "—"}</span>
                <span class="badge">{repo.private ? "private" : "public"}</span>
                {#if repo.archived}<span class="badge warn">archived</span>{/if}
                {#if repo.fork}<span class="badge">fork</span>{/if}
              </div>
              {#if repo.description}
                <p class="repo-desc">{repo.description}</p>
              {/if}
              <div class="repo-facts mono">
                <span>{repo.language ?? "no language"}</span>
                {#each (repo.topics ?? []).slice(0, 3) as topic}
                  <span class="topic-fact">#{topic}</span>
                {/each}
                <span>★ {repo.stargazersCount}</span>
                <span>Y {repo.forksCount}</span>
                <span>{repo.defaultBranch}</span>
                <span>updated {relativeTime(Date.parse(repo.updatedAt) / 1000)}</span>
              </div>
            </div>
            <div class="repo-actions">
              <button
                class="btn ghost icon"
                title="Browse in GRAFT"
                aria-label={`Browse ${repo.fullName} in GRAFT`}
                onclick={() => openRepo(repo)}
              >
                <Icon name="chevron-right" size={13} />
              </button>
              <button
                class="btn ghost icon"
                title="Open on GitHub"
                aria-label={`Open ${repo.fullName} on GitHub`}
                onclick={() => open(repo.htmlUrl)}
              >
                <Icon name="arrow-up-right" size={13} />
              </button>
            </div>
          </li>
        {/each}
      </ul>

      {#if repoScope === "mine" && github.hasMore}
        <button class="btn load-more" onclick={() => loadRepos(false)} disabled={github.reposLoading}>
          {github.reposLoading ? "Loading…" : "Load more repositories"}
        </button>
      {:else if repoScope === "explore" && github.discover.hasMore}
        <button class="btn load-more" onclick={() => searchRepos(false)} disabled={github.discover.loading}>
          {github.discover.loading ? "Loading…" : "Load more results"}
        </button>
      {/if}
    {/if}
  {/if}
</section>

<style>
  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .auth-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--gap);
    align-items: start;
  }

  .auth {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .auth-head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding-bottom: 9px;
    border-bottom: 1px solid var(--border);
  }

  .step {
    font-size: 10px;
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    background: var(--accent-soft);
    border-radius: 2px;
    padding: 1px 4px;
    line-height: 14px;
  }

  .auth-head h2 {
    font-size: var(--font-small);
    font-weight: 600;
  }

  .auth-head p {
    margin-top: 2px;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-faint);
    max-width: 46ch;
  }

  .field-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .code-block {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    border-radius: var(--radius);
  }

  .code {
    font-family: var(--font-mono);
    font-size: 21px;
    letter-spacing: 0.22em;
    color: var(--accent);
    background: transparent;
    border: 0;
    padding: 0;
    text-align: left;
    cursor: pointer;
    user-select: text;
  }

  .waiting {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 9.5px;
  }

  .pulse {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--warn);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }

  .hint {
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .hint .mono {
    color: var(--text-muted);
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
    line-height: 1.5;
    color: var(--danger);
  }

  .wide {
    width: 100%;
  }

  .token {
    margin-top: var(--gap);
    padding: 0 12px 10px;
  }

  .token-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 30px;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
  }

  .token-head:hover .mono-label {
    color: var(--text-muted);
  }

  .token-body {
    display: flex;
    gap: 6px;
    margin-bottom: 7px;
  }

  .account {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 2px 10px;
  }

  .identity {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .avatar {
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    object-fit: cover;
    background: var(--bg-elev);
  }

  .who {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .login {
    font-size: var(--font-small);
    font-weight: 600;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 8px 0 10px;
    margin-bottom: 6px;
    color: var(--text-faint);
  }

  .scope-switch {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex: none;
  }

  .scope-btn {
    height: 22px;
    padding: 0 8px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .scope-btn:last-child {
    border-right: 0;
  }

  .scope-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .discover-form {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
  }

  .discover-form .btn {
    height: 22px;
    padding: 0 8px;
    font-size: 9.5px;
    flex: none;
  }

  .discover {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 8px 10px;
    margin-bottom: 6px;
  }

  .discover-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
  }

  .discover-grid .field {
    height: 24px;
    font-size: 10px;
  }

  .discover-topics {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
  }

  .topic-chip {
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .topic-chip:hover,
  .topic-chip.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }

  .discover-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }

  .discover-foot .hint {
    max-width: 80ch;
  }

  .discover .link {
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .discover .link:hover {
    color: var(--accent);
  }

  .search {
    flex: 1;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--font-small);
    outline: none;
    min-width: 0;
  }

  .search::placeholder {
    color: var(--text-faint);
  }

  .view-switch {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex: none;
  }

  .view-btn {
    height: 22px;
    padding: 0 7px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .view-btn:last-child {
    border-right: 0;
  }

  .view-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .repos {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .repos.cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    align-items: stretch;
  }

  .repos.cards .repo {
    align-items: flex-start;
    grid-template-columns: minmax(0, 1fr) auto;
    padding: 10px 9px 9px 11px;
  }

  .repos.cards .repo-desc {
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    white-space: normal;
    overflow: hidden;
    line-height: 1.45;
    max-width: none;
  }

  .repos.cards .repo-facts {
    flex-wrap: wrap;
    white-space: normal;
    gap: 6px 9px;
    margin-top: 2px;
  }

  .repos.compact .repo {
    padding: 4px 8px 4px 10px;
  }

  .repos.compact .repo-desc {
    display: none;
  }

  .repos.compact .repo-title {
    gap: 5px;
  }

  .repos.compact .repo-facts {
    gap: 8px;
  }

  @media (max-width: 900px) {
    .discover-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 700px) {
    .repos.cards {
      grid-template-columns: minmax(0, 1fr);
    }

    .view-switch {
      display: none;
    }

    .toolbar {
      flex-wrap: wrap;
      height: auto;
      padding: 6px 8px;
    }

    .discover-form {
      flex-basis: 100%;
    }
  }

  .repo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 11px;
    transition: border-color var(--motion);
  }

  .repo:hover {
    border-color: var(--border-strong);
  }

  .repo-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .repo-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .repo-title {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    color: var(--text-faint);
  }

  .repo-name {
    font-size: var(--font-small);
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .owner {
    font-size: 9.5px;
    color: var(--text-faint);
  }

  .repo-desc {
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 96ch;
  }

  .repo-facts {
    display: flex;
    gap: 12px;
    font-size: 9px;
    color: var(--text-faint);
    overflow: hidden;
    white-space: nowrap;
  }

  .topic-fact {
    color: color-mix(in srgb, var(--accent) 75%, var(--text-faint));
  }

  .notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 12px;
    margin-bottom: var(--gap);
    font-size: var(--font-small);
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
    max-width: 64ch;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .empty .btn {
    margin-top: 5px;
  }

  .load-more {
    margin-top: 7px;
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

  @media (max-width: 1080px) {
    .auth-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
