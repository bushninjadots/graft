<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Project } from "../lib/api";
  import TerminalPane from "../lib/components/TerminalPane.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import {
    closeTerminal,
    focusTerminal,
    loadTerminalEnvironment,
    startTerminal,
    terminal,
  } from "../lib/state/terminal.svelte";
  import { navigate } from "../lib/state/ui.svelte";

  let projects = $state<Project[]>([]);
  let cwd = $state("");
  let shell = $state("");
  let formOpen = $state(false);

  const active = $derived(
    terminal.sessions.find((session) => session.info.id === terminal.activeId) ?? null,
  );

  onMount(() => {
    void (async () => {
      if (!terminal.env) await loadTerminalEnvironment();
      cwd = terminal.env?.cwd ?? "";
      shell = terminal.env?.defaultShell ?? "";
      projects = await api.listProjects().catch(() => []);
      if (terminal.sessions.length === 0) formOpen = true;
    })();
  });

  function label(path: string): string {
    const parts = path.split("/").filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  function shellLabel(path: string): string {
    return path.split("/").pop() ?? path;
  }

  function chooseProject(event: Event): void {
    const value = (event.currentTarget as HTMLSelectElement).value;
    if (value === "custom") return;
    cwd = value;
  }

  async function launch(): Promise<void> {
    const id = await startTerminal(cwd, shell);
    if (id) formOpen = false;
  }

  function close(id: string): void {
    void closeTerminal(id);
  }
</script>

<section class="view terminal-view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Terminal</h1>
      <p class="view-sub">
        Real PTY sessions owned by the Rust engine. Choose a linked project directory and a shell;
        tabs stay alive while you work elsewhere in GRAFT.
      </p>
    </div>
    <div class="head-actions">
      <span class="badge">{terminal.sessions.length} session{terminal.sessions.length === 1 ? "" : "s"}</span>
      <button class="btn" onclick={() => (formOpen = !formOpen)}>
        <Icon name="plus" size={13} />
        {formOpen ? "Close launcher" : "New terminal"}
      </button>
    </div>
  </div>

  {#if terminal.error}
    <div class="panel notice">
      <span class="badge warn">terminal error</span>
      <p>{terminal.error}</p>
    </div>
  {/if}

  {#if formOpen}
    <div class="panel launcher">
      <div class="launcher-head">
        <span class="mono-label">new session</span>
        <span class="hint">Nothing is started until you press Start.</span>
      </div>
      <div class="launcher-grid">
        <label class="field-label">
          <span class="mono faint">working directory</span>
          <input class="field mono" bind:value={cwd} spellcheck="false" placeholder="/home/you" />
        </label>
        <label class="field-label">
          <span class="mono faint">linked project</span>
          <select class="field" onchange={chooseProject}>
            <option value="custom">Custom path</option>
            {#each projects.filter((project) => project.path) as project (project.id)}
              <option value={project.path}>{project.name} · {project.path}</option>
            {/each}
          </select>
        </label>
        <label class="field-label">
          <span class="mono faint">shell</span>
          <select class="field" bind:value={shell}>
            {#each terminal.env?.shells ?? [] as item}
              <option value={item}>{shellLabel(item)} · {item}</option>
            {/each}
          </select>
        </label>
        <div class="launch">
          <button class="btn primary" onclick={launch} disabled={terminal.starting || !cwd || !shell}>
            {terminal.starting ? "Starting…" : "Start terminal"}
          </button>
        </div>
      </div>
      {#if projects.length === 0}
        <p class="hint">
          No linked project directories yet.
          <button class="link" onclick={() => navigate("projects")}>Add a project</button> to give the
          terminal a one-click working directory.
        </p>
      {:else}
        <p class="hint">Session working directories are validated in Rust before the shell starts.</p>
      {/if}
    </div>
  {/if}

  {#if terminal.sessions.length === 0}
    <div class="panel empty">
      <Icon name="terminal" size={24} />
      <p class="empty-title">No terminal session yet</p>
      <p class="empty-copy">
        Start a session above. fish is preferred when installed, otherwise GRAFT uses your login
        shell or bash. Sessions use a native PTY, not a simulated prompt.
      </p>
      {#if !terminal.loading && terminal.env}
        <button class="btn" onclick={() => (formOpen = true)}>Open launcher</button>
      {/if}
    </div>
  {:else}
    <div class="workspace panel">
      <div class="tabs" role="tablist" aria-label="Terminal sessions">
        {#each terminal.sessions as session (session.info.id)}
          <div class="tab" class:active={terminal.activeId === session.info.id}>
            <button
              class="tab-main"
              role="tab"
              aria-selected={terminal.activeId === session.info.id}
              onclick={() => focusTerminal(session.info.id)}
              title={session.info.cwd}
            >
              <Icon name="terminal" size={11} />
              <span class="tab-name">{shellLabel(session.info.shell)} · {label(session.info.cwd)}</span>
              {#if session.ended}<span class="ended mono-label">exited</span>{/if}
            </button>
            <button
              class="tab-close"
              aria-label={`Close ${shellLabel(session.info.shell)} session`}
              onclick={() => close(session.info.id)}
            >
              <Icon name="x" size={10} />
            </button>
          </div>
        {/each}
        <button class="tab-add" title="New terminal" aria-label="New terminal" onclick={() => (formOpen = true)}>
          <Icon name="plus" size={12} />
        </button>
        <span class="tab-meta mono-label">
          {#if active}
            {active.info.cwd} · {active.info.shell}
          {:else}
            select a session
          {/if}
        </span>
      </div>

      <div class="panes">
        {#each terminal.sessions as session (session.info.id)}
          <div class="pane-slot" class:hidden={terminal.activeId !== session.info.id}>
            <TerminalPane {session} active={terminal.activeId === session.info.id} />
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <p class="footnote">
    Ctrl+Shift+C / Ctrl+Shift+V use the webview clipboard; the terminal also supports mouse
    selection. Terminal processes run with your user permissions — GRAFT does not sandbox them.
  </p>
</section>

<style>
  .view.terminal-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 10px;
    margin-bottom: var(--gap);
    font-size: 10.5px;
  }

  .launcher {
    padding: 9px 11px 10px;
    margin-bottom: var(--gap);
  }

  .launcher-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding-bottom: 7px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }

  .launcher-grid {
    display: grid;
    grid-template-columns: minmax(200px, 1.5fr) minmax(150px, 1fr) minmax(150px, 0.9fr) auto;
    align-items: end;
    gap: 7px;
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .field {
    height: 25px;
    font-size: 10px;
  }

  .launch {
    display: flex;
    align-items: flex-end;
  }

  .launch .btn {
    height: 25px;
    font-size: 10px;
  }

  .hint {
    margin-top: 6px;
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .link:hover {
    color: var(--accent);
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
    max-width: 68ch;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .workspace {
    flex: 1;
    min-height: 320px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .tabs {
    display: flex;
    align-items: stretch;
    gap: 2px;
    min-height: 29px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-alt);
    overflow-x: auto;
  }

  .tab {
    display: flex;
    align-items: center;
    border-right: 1px solid var(--border);
    background: transparent;
    flex: none;
  }

  .tab.active {
    background: var(--bg-elev);
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .tab-main {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 29px;
    padding: 0 8px;
    border: 0;
    background: none;
    color: var(--text-faint);
    font: inherit;
    font-size: 9.5px;
    cursor: pointer;
    max-width: 230px;
  }

  .tab.active .tab-main {
    color: var(--text);
  }

  .tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ended {
    font-size: 8px;
    color: var(--danger);
  }

  .tab-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    margin-right: 3px;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }

  .tab-close:hover {
    color: var(--danger);
    background: var(--surface);
  }

  .tab-add {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 27px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    flex: none;
  }

  .tab-add:hover {
    color: var(--text);
    background: var(--surface);
  }

  .tab-meta {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    padding: 0 9px;
    font-size: 8px;
    white-space: nowrap;
    max-width: 42%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .panes {
    flex: 1;
    min-height: 0;
    background: var(--bg-elev);
  }

  .pane-slot {
    height: 100%;
    min-height: 0;
  }

  .pane-slot.hidden {
    display: none;
  }

  .footnote {
    margin-top: 8px;
    font-size: 9px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  @media (max-width: 900px) {
    .launcher-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .launch .btn {
      width: 100%;
    }

    .tab-meta {
      display: none;
    }
  }
</style>
