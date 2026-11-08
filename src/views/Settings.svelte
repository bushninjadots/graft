<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { NAV_ITEMS, navById } from "../lib/nav";
  import { themes } from "../lib/themes";
  import { plural } from "../lib/format";
  import {
    appearance,
    setDensity,
    setRadius,
    setTheme,
    type Density,
  } from "../lib/state/appearance.svelte";
  import { moveNav, navState, resetNav, toggleHidden, togglePinned } from "../lib/state/nav.svelte";
  import { dashboard, resetDashboard } from "../lib/state/dashboard.svelte";
  import { refreshTools, session } from "../lib/state/session.svelte";
  import {
    github,
    githubUser,
    isSignedIn,
    loadGithub,
    setClientId,
    signOut,
  } from "../lib/state/github.svelte";
  import Icon from "../lib/components/Icon.svelte";

  const densities: { id: Density; label: string; hint: string }[] = [
    { id: "compact", label: "Compact", hint: "26 px rows" },
    { id: "comfortable", label: "Comfortable", hint: "30 px rows" },
    { id: "roomy", label: "Roomy", hint: "34 px rows" },
  ];

  const orderedItems = $derived(
    navState.order.map((id) => navById(id)).filter((item) => !!item),
  );

  const foundCount = $derived(session.tools.filter((tool) => tool.found).length);

  const account = $derived(githubUser());
  const connected = $derived(isSignedIn());
  let clientIdDraft = $state("");
  let clientIdEditing = $state(false);
  let savingClientId = $state(false);

  $effect(() => {
    if (!clientIdEditing) clientIdDraft = github.status?.clientId ?? "";
  });

  onMount(() => {
    if (!github.status && !github.loading) void loadGithub();
  });

  async function saveClientId(): Promise<void> {
    savingClientId = true;
    if (await setClientId(clientIdDraft)) clientIdEditing = false;
    savingClientId = false;
  }

  function openLink(url: string): void {
    void api.openExternal(url).catch(() => {});
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Settings</h1>
      <p class="view-sub">Everything here is stored locally. No account is required to customise GRAFT.</p>
    </div>
  </div>

  <!-- Appearance ------------------------------------------------------- -->
  <section class="block">
    <div class="block-head">
      <h2>Appearance</h2>
      <span class="mono-label">themes apply instantly</span>
    </div>

    <div class="theme-grid">
      {#each themes as theme (theme.id)}
        <button
          class="theme"
          class:active={appearance.themeId === theme.id}
          aria-pressed={appearance.themeId === theme.id}
          onclick={() => setTheme(theme.id)}
        >
          <span class="preview" style:background={theme.tokens.bg} style:border-color={theme.tokens.border}>
            <span class="rail" style:background={theme.tokens.bgElev}></span>
            <span class="chip" style:background={theme.tokens.surface} style:border-color={theme.tokens.border}></span>
            <span class="chip alt" style:background={theme.tokens.surfaceAlt} style:border-color={theme.tokens.border}></span>
            <span class="accent" style:background={theme.tokens.accent}></span>
          </span>
          <span class="theme-text">
            <span class="theme-name">
              {theme.name}
              {#if appearance.themeId === theme.id}<span class="check"><Icon name="check" size={11} /></span>{/if}
            </span>
            <span class="theme-blurb mono">{theme.blurb}</span>
          </span>
          <span class="scheme mono-label">{theme.tokens.scheme}</span>
        </button>
      {/each}
    </div>

    <div class="controls">
      <div class="control">
        <span class="mono-label">Density</span>
        <div class="segmented">
          {#each densities as density (density.id)}
            <button
              class:active={appearance.density === density.id}
              onclick={() => setDensity(density.id)}
              title={density.hint}
            >
              {density.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="control">
        <span class="mono-label">Corner radius · {appearance.radius}px</span>
        <input
          class="slider"
          type="range"
          min="0"
          max="10"
          step="1"
          value={appearance.radius}
          oninput={(event) => setRadius(Number(event.currentTarget.value))}
          aria-label="Corner radius"
        />
      </div>
    </div>
  </section>

  <!-- Navigation ------------------------------------------------------- -->
  <section class="block">
    <div class="block-head">
      <h2>Navigation</h2>
      <div class="head-actions">
        <span class="mono-label">{plural(NAV_ITEMS.length - navState.hidden.length, "section")} visible</span>
        <button class="btn ghost" onclick={resetNav}>Reset</button>
      </div>
    </div>

    <div class="panel nav-list">
      {#each orderedItems as item (item.id)}
        <div class="nav-row" class:dim={navState.hidden.includes(item.id)}>
          <span class="nav-icon"><Icon name={item.id} size={14} /></span>
          <span class="nav-label">{item.label}</span>
          <span class="nav-group mono-label">{item.group}</span>
          <span class="nav-actions">
            <button
              class="btn ghost icon"
              class:on={navState.pinned.includes(item.id)}
              title={navState.pinned.includes(item.id) ? "Unpin" : "Pin to top"}
              aria-label={navState.pinned.includes(item.id) ? `Unpin ${item.label}` : `Pin ${item.label}`}
              onclick={() => togglePinned(item.id)}
            >
              <Icon name="pin" size={12} />
            </button>
            <button
              class="btn ghost icon"
              title={navState.hidden.includes(item.id) ? "Show in sidebar" : "Hide from sidebar"}
              aria-label={`Toggle ${item.label} visibility`}
              onclick={() => toggleHidden(item.id)}
            >
              <Icon name={navState.hidden.includes(item.id) ? "eye-off" : "eye"} size={13} />
            </button>
            <button
              class="btn ghost icon"
              aria-label={`Move ${item.label} up`}
              onclick={() => moveNav(item.id, -1)}
              disabled={navState.order.indexOf(item.id) === 0}
            >
              <Icon name="chevron-up" size={12} />
            </button>
            <button
              class="btn ghost icon"
              aria-label={`Move ${item.label} down`}
              onclick={() => moveNav(item.id, 1)}
              disabled={navState.order.indexOf(item.id) === navState.order.length - 1}
            >
              <Icon name="chevron-down" size={12} />
            </button>
          </span>
        </div>
      {/each}
    </div>
    <p class="hint">Hidden modules stay reachable from the command palette (Ctrl K).</p>
  </section>

  <!-- Dashboard -------------------------------------------------------- -->
  <section class="block">
    <div class="block-head">
      <h2>Dashboard</h2>
      <span class="mono-label">{plural(dashboard.hidden.length, "module")} hidden</span>
    </div>
    <div class="panel inline-panel">
      <div>
        <p class="inline-title">Module layout</p>
        <p class="hint">Reorder modules by dragging them on Home while Customize is active.</p>
      </div>
      <button class="btn" onclick={resetDashboard}>Reset layout</button>
    </div>
  </section>

  <!-- Environment ------------------------------------------------------ -->
  <section class="block">
    <div class="block-head">
      <h2>Environment</h2>
      <div class="head-actions">
        <span class="mono-label">{foundCount} of {session.tools.length} detected</span>
        <button class="btn ghost" onclick={() => refreshTools()} disabled={!session.online || session.toolsLoading}>
          {session.toolsLoading ? "Scanning…" : "Re-scan"}
        </button>
      </div>
    </div>
    <div class="panel tools">
      {#if session.tools.length === 0}
        <p class="hint pad">Nothing scanned yet.</p>
      {:else}
        {#each session.tools as tool (tool.id)}
          <div class="tool">
            <span class="dot" class:on={tool.found}></span>
            <span class="tool-name">{tool.name}</span>
            <span class="tool-version mono">{tool.version ?? "not installed"}</span>
            <span class="tool-path mono" title={tool.path ?? ""}>{tool.path ?? ""}</span>
          </div>
        {/each}
      {/if}
    </div>
    <p class="hint">
      GRAFT reports what is installed. It never installs packages or changes system configuration.
    </p>
  </section>

  <!-- GitHub ------------------------------------------------------------ -->
  <section class="block">
    <div class="block-head">
      <h2>GitHub</h2>
      <span class="mono-label">
        {connected ? `signed in as ${account?.login ?? "github"}` : "not connected"}
      </span>
    </div>

    <div class="panel facts">
      <div>
        <span>Token storage</span>
        <span class="mono">
          {github.status?.keyringError ? "keyring unavailable" : "system keyring · secret service"}
        </span>
      </div>
      <div><span>Scopes</span><span class="mono">{github.status?.scopes ?? "none requested"}</span></div>
      <div><span>Account</span><span class="mono">{account?.login ?? "—"}</span></div>
      <div><span>OAuth client</span><span class="mono">{github.status?.clientId ?? "not configured"}</span></div>
    </div>

    <div class="inline-panel panel github-actions">
      <div class="client">
        <span class="mono-label">OAuth app client ID</span>
        <input
          class="field mono"
          bind:value={clientIdDraft}
          oninput={() => (clientIdEditing = true)}
          placeholder="20 characters"
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div class="head-actions">
        <button class="btn" onclick={saveClientId} disabled={savingClientId}>
          {savingClientId ? "Saving…" : "Save"}
        </button>
        <button class="btn ghost" onclick={() => openLink("https://github.com/settings/developers")}>
          Create OAuth app ↗
        </button>
        {#if connected}
          <button class="btn ghost" onclick={() => signOut()}>Sign out</button>
        {/if}
      </div>
    </div>

    {#if github.status?.keyringError}
      <p class="hint tone-danger">{github.status.keyringError}</p>
    {/if}
    {#if github.status?.tokenError}
      <p class="hint tone-danger">{github.status.tokenError}</p>
    {/if}
    <p class="hint">
      The token is written to the Secret Service and never to SQLite, logs or this window.
      Sign-in and repository browsing live in Repositories.
    </p>
  </section>

  <!-- Data ------------------------------------------------------------- -->
  <section class="block">
    <div class="block-head">
      <h2>Data</h2>
      <span class="mono-label">local sqlite</span>
    </div>
    <div class="panel facts">
      <div><span>Version</span><span class="mono">{session.info?.version ?? "—"}</span></div>
      <div><span>Schema</span><span class="mono">{session.info?.schemaVersion ?? "—"}</span></div>
      <div><span>Data directory</span><span class="mono selectable">{session.info?.dataDir ?? "—"}</span></div>
      <div><span>Database</span><span class="mono selectable">{session.info?.dbPath ?? "—"}</span></div>
      <div><span>Engine</span><span class="mono">{session.online ? "connected" : "offline"}</span></div>
    </div>
    {#if !session.online}
      <p class="hint tone-danger">
        {session.error ?? "The Rust backend is unreachable."} Run <span class="mono">npm run app:dev</span>.
      </p>
    {/if}
  </section>

  <!-- About ------------------------------------------------------------ -->
  <section class="block">
    <div class="block-head">
      <h2>About</h2>
      <span class="mono-label">phase 1 · github</span>
    </div>
    <div class="panel about">
      <p>
        GRAFT is a personal GitHub environment: repositories, projects, review, monitoring and
        extraction in one lightweight desktop application. Built with Tauri 2, Rust, Svelte 5 and
        SQLite.
      </p>
      <p class="mono-label">
        Blueprint: docs/PLAN.md · Architecture: docs/ARCHITECTURE.md · Design: docs/DESIGN.md
      </p>
    </div>
  </section>
</section>

<style>
  .block {
    margin-bottom: 22px;
    max-width: 1040px;
  }

  .block-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 7px;
    padding-bottom: 5px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    font-size: var(--font-small);
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 7px;
  }

  .theme {
    display: grid;
    grid-template-columns: 52px 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 7px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    text-align: left;
    cursor: pointer;
    transition:
      border-color var(--motion),
      background var(--motion);
  }

  .theme:hover {
    border-color: var(--border-strong);
    background: var(--surface-alt);
  }

  .theme.active {
    border-color: color-mix(in srgb, var(--accent) 65%, var(--border));
    background: var(--surface-alt);
  }

  .preview {
    position: relative;
    display: block;
    height: 38px;
    border: 1px solid;
    border-radius: 2px;
    overflow: hidden;
  }

  .preview .rail {
    position: absolute;
    inset: 0 auto 0 0;
    width: 13px;
  }

  .preview .chip {
    position: absolute;
    left: 18px;
    right: 6px;
    height: 9px;
    border: 1px solid;
    border-radius: 1px;
  }

  .preview .chip {
    top: 6px;
  }

  .preview .chip.alt {
    top: 19px;
    right: 16px;
  }

  .preview .accent {
    position: absolute;
    left: 18px;
    top: 33px;
    width: 22px;
    height: 2px;
  }

  .theme-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .theme-name {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--font-small);
    font-weight: 600;
    color: var(--text);
  }

  .check {
    display: inline-flex;
    color: var(--accent);
  }

  .theme-blurb {
    font-size: 9px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scheme {
    font-size: 8.5px;
    align-self: flex-start;
    margin-top: 2px;
  }

  .controls {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
    margin-top: 12px;
  }

  .control {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .segmented {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    width: fit-content;
    background: var(--surface);
  }

  .segmented button {
    height: 25px;
    padding: 0 10px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-muted);
    font-size: 10.5px;
    cursor: pointer;
    transition:
      background var(--motion),
      color var(--motion);
  }

  .segmented button:last-child {
    border-right: 0;
  }

  .segmented button:hover {
    background: var(--surface-alt);
    color: var(--text);
  }

  .segmented button.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .slider {
    width: 100%;
    max-width: 260px;
    accent-color: var(--accent);
    height: 18px;
  }

  .nav-list {
    padding: 3px 0;
  }

  .nav-row {
    display: grid;
    grid-template-columns: 18px minmax(110px, 1fr) 96px auto;
    align-items: center;
    gap: 8px;
    height: calc(var(--row-h) + 2px);
    padding: 0 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  }

  .nav-row:last-child {
    border-bottom: 0;
  }

  .nav-row.dim .nav-label,
  .nav-row.dim .nav-icon {
    opacity: 0.4;
  }

  .nav-icon {
    display: inline-flex;
    color: var(--text-faint);
  }

  .nav-label {
    font-size: var(--font-small);
  }

  .nav-group {
    font-size: 9px;
  }

  .nav-actions {
    display: inline-flex;
    gap: 1px;
  }

  .nav-actions .btn {
    width: 24px;
    height: 22px;
  }

  .nav-actions .btn.on {
    color: var(--accent);
  }

  .inline-panel {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 10px 12px;
  }

  .github-actions {
    margin-top: 8px;
    align-items: flex-end;
  }

  .client {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
    max-width: 340px;
  }

  .inline-title {
    font-size: var(--font-small);
  }

  .hint {
    margin-top: 6px;
    font-size: 10px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .hint.pad {
    padding: 10px 12px;
    margin: 0;
  }

  .tools {
    padding: 3px 0;
  }

  .tool {
    display: grid;
    grid-template-columns: 10px 132px 1fr minmax(0, 1.6fr);
    align-items: center;
    gap: 8px;
    height: 24px;
    padding: 0 10px;
    font-size: 10.5px;
  }

  .tool:hover {
    background: var(--surface-alt);
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--border-strong);
  }

  .dot.on {
    background: var(--ok);
  }

  .tool-name {
    color: var(--text-muted);
  }

  .tool-version {
    font-size: 9.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tool-path {
    font-size: 9px;
    color: var(--text-faint);
    opacity: 0.7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .facts div {
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 10px;
    padding: 5px 12px;
    font-size: var(--font-small);
    color: var(--text-muted);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
  }

  .facts div:last-child {
    border-bottom: 0;
  }

  .facts div span:first-child {
    color: var(--text-faint);
    font-size: 10.5px;
  }

  .selectable {
    user-select: text;
    font-size: 10px;
  }

  .about {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: var(--font-small);
    color: var(--text-muted);
    line-height: 1.6;
    max-width: 78ch;
  }

  @media (max-width: 860px) {
    .nav-row {
      grid-template-columns: 18px 1fr auto;
    }
    .nav-group {
      display: none;
    }
    .tool {
      grid-template-columns: 10px 110px 1fr;
    }
    .tool-path {
      display: none;
    }
  }
</style>
