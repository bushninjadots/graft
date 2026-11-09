<script lang="ts">
  import { onMount } from "svelte";
  import { api, type PluginRecord, type RegistryPlugin } from "../lib/api";
  import {
    install,
    loadPlugins,
    plugins,
    refreshRegistry,
    removeKey,
    saveRegistryUrl,
    setEnabled,
    uninstall,
  } from "../lib/state/plugins.svelte";
  import { allThemes } from "../lib/state/themeCatalog.svelte";
  import { reconcileTheme } from "../lib/state/appearance.svelte";
  import { refreshTools, session } from "../lib/state/session.svelte";

  type ToolRow = { id: string; label: string; role: string };
  type IntegrationNote = { plugin: string; notes: string; docsUrl: string | null };

  const phaseTools: ToolRow[] = [
    { id: "ollama", label: "Ollama", role: "used by the AI workspace" },
    { id: "docker", label: "Docker", role: "container tooling" },
    { id: "gh", label: "GitHub CLI", role: "repository operations" },
    { id: "rust-analyzer", label: "rust-analyzer", role: "language server" },
    { id: "claude", label: "Claude Code", role: "agentic coding" },
    { id: "opencode", label: "OpenCode", role: "agentic coding" },
  ];

  const registrySchema = `{
  "schema": 1,
  "name": "Example Registry",
  "updatedAt": "2026-11-09T12:00:00Z",
  "plugins": [
    {
      "id": "dev.example.night-owl",
      "name": "Night Owl",
      "version": "1.2.0",
      "kind": "theme",
      "summary": "A low-glare theme for late sessions.",
      "manifestUrl": "https://example.dev/night-owl/plugin.json",
      "manifestSha256": "64 hex characters",
      "signatureUrl": "https://example.dev/night-owl/plugin.json.sig",
      "publicKey": "base64 of 32 raw Ed25519 bytes"
    }
  ]
}`;

  let registryDraft = $state("");
  let registryDraftDirty = $state(false);
  let selected = $state<RegistryPlugin | null>(null);
  let trust = $state(false);
  let registryRequested = false;

  const registryUrl = $derived(plugins.registryUrl.trim());
  const registryCount = $derived(plugins.registry?.plugins.length ?? 0);
  const themeCount = $derived(allThemes().length);

  const installedGroups = $derived([
    {
      id: "theme",
      label: "Themes",
      items: plugins.installed.filter((plugin) => plugin.kind === "theme"),
    },
    {
      id: "integration",
      label: "Integrations",
      items: plugins.installed.filter((plugin) => plugin.kind === "integration"),
    },
  ]);

  const selectedFingerprint = $derived(selected?.fingerprint ?? "");
  const keyTrusted = $derived(
    selectedFingerprint !== "" &&
      plugins.keys.some((key) => key.fingerprint === selectedFingerprint),
  );
  const canInstall = $derived(
    !!selected && !plugins.busyId && (keyTrusted || trust),
  );
  const selectedInstalled = $derived(
    selected ? plugins.installed.some((plugin) => plugin.id === selected!.id) : false,
  );
  const selectedCapability = $derived(
    selected?.kind === "theme"
      ? "theme.apply · applies one theme"
      : selected
        ? "integration.inspect · adds notes for detected tools"
        : "",
  );

  const toolMap = $derived(new Map(session.tools.map((tool) => [tool.id, tool])));
  const notesByTool = $derived.by(() => {
    const map = new Map<string, IntegrationNote[]>();
    for (const plugin of plugins.installed) {
      if (
        plugin.kind !== "integration" ||
        !plugin.enabled ||
        !plugin.verified ||
        !plugin.integration
      ) {
        continue;
      }
      for (const tool of plugin.integration.tools) {
        const list = map.get(tool) ?? [];
        list.push({
          plugin: plugin.name,
          notes: plugin.integration.notes,
          docsUrl: plugin.integration.docsUrl ?? null,
        });
        map.set(tool, list);
      }
    }
    return map;
  });
  const otherTools = $derived(
    session.tools.filter(
      (tool) => tool.found && !phaseTools.some((item) => item.id === tool.id),
    ).length,
  );

  $effect(() => {
    const saved = plugins.registryUrl;
    if (!registryDraftDirty && registryDraft !== saved) registryDraft = saved;
  });

  onMount(() => {
    void (async () => {
      if (!plugins.loaded && !plugins.loading) await loadPlugins();
      if (!registryDraftDirty) registryDraft = plugins.registryUrl;
      if (!registryRequested && plugins.registryUrl.trim() && !plugins.registry) {
        registryRequested = true;
        await refreshRegistry();
      }
    })();
    if (!session.tools.length) void refreshTools();
  });

  function grouped(fingerprint: string): string {
    return fingerprint.match(/.{1,4}/g)?.join(" ") ?? fingerprint;
  }

  function openDocs(url: string): void {
    void api.openExternal(url).catch(() => {});
  }

  async function saveRegistry(): Promise<void> {
    saveRegistryUrl(registryDraft);
    registryDraftDirty = false;
    registryDraft = plugins.registryUrl;
    selected = null;
    trust = false;
    registryRequested = true;
    await refreshRegistry();
  }

  async function reloadRegistry(): Promise<void> {
    selected = null;
    trust = false;
    registryRequested = true;
    await refreshRegistry();
  }

  function selectEntry(entry: RegistryPlugin): void {
    selected = entry;
    trust = false;
    plugins.installError = null;
  }

  async function installSelected(): Promise<void> {
    const entry = selected;
    if (!entry || !canInstall) return;
    const record = await install(entry, !keyTrusted);
    if (record) {
      selected = null;
      trust = false;
      reconcileTheme();
    }
  }

  async function toggle(record: PluginRecord): Promise<void> {
    await setEnabled(record.id, !record.enabled);
    reconcileTheme();
  }

  async function removePlugin(record: PluginRecord): Promise<void> {
    if (!confirm(`Uninstall “${record.name}”?`)) return;
    await uninstall(record.id);
    reconcileTheme();
  }

  async function removeTrustedKey(fingerprint: string, label: string): Promise<void> {
    if (!confirm(`Remove the signing key “${label || grouped(fingerprint)}”?`)) return;
    await removeKey(fingerprint);
    reconcileTheme();
  }
</script>

<section class="view plugins-view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Plugin Marketplace</h1>
      <p class="view-sub">
        Secured by signatures. Themes and integration notes are declarative packages; GRAFT
        verifies every one and never runs plugin code.
      </p>
    </div>
    <div class="head-actions">
      <span class="badge ok">signed packages</span>
      <span class="badge">{themeCount} themes</span>
    </div>
  </div>

  {#if plugins.error}
    <p class="banner danger">{plugins.error}</p>
  {/if}

  <div class="layout">
    <div class="column">
      <section class="panel section registry">
        <div class="section-head">
          <span class="mono-label">registry</span>
          {#if plugins.registryLoading}
            <span class="badge accent">loading</span>
          {:else if plugins.registry}
            <span class="badge ok">{registryCount} packages</span>
          {/if}
        </div>

        <div class="registry-row">
          <input
            class="field mono"
            bind:value={registryDraft}
            oninput={() => (registryDraftDirty = true)}
            placeholder="https://registry.example/index.json"
            aria-label="Registry URL"
          />
          <button class="btn" onclick={() => void saveRegistry()}>Save</button>
          <button
            class="btn ghost"
            onclick={() => void reloadRegistry()}
            disabled={!registryUrl || plugins.registryLoading}
          >
            Refresh
          </button>
        </div>

        {#if plugins.registryError}
          <p class="state danger" role="alert">{plugins.registryError}</p>
        {:else if plugins.registry}
          <p class="state">
            <span class="strong">{plugins.registry.name}</span>
            <span class="faint mono"> · updated {plugins.registry.updatedAt}</span>
          </p>
        {:else if !registryUrl}
          <p class="state faint">No registry configured. Installed plugins still load from this machine.</p>
        {:else if !plugins.registryLoading}
          <p class="state faint">Press Refresh to read this registry.</p>
        {/if}

        <details class="schema">
          <summary class="mono-label">registry schema 1</summary>
          <p class="schema-copy">
            A registry is a JSON index served over HTTPS (plain HTTP is accepted for
            loopback development). It is a discovery document, not a trust anchor.
          </p>
          <pre class="mono">{registrySchema}</pre>
        </details>
      </section>

      <section class="panel section">
        <div class="section-head">
          <span class="mono-label">installed</span>
          <span class="badge">{plugins.installed.length}</span>
        </div>

        {#if !plugins.loaded && plugins.loading}
          <p class="state mono faint">reading local plugins…</p>
        {:else if plugins.installed.length === 0}
          <p class="empty-copy">
            Nothing is installed. Add a registry on the left, or keep GRAFT's built-in
            themes and environment detection as they are.
          </p>
        {:else}
          {#each installedGroups as group (group.id)}
            <div class="group">
              <div class="group-head">
                <span class="mono-label">{group.label}</span>
                <span class="count mono">{group.items.length}</span>
              </div>
              {#if group.items.length === 0}
                <p class="state faint">None installed.</p>
              {:else}
                {#each group.items as plugin (plugin.id)}
                  <article class="installed" class:off={!plugin.enabled}>
                    <div class="installed-head">
                      <div class="installed-name">
                        <span class="strong">{plugin.name}</span>
                        <span class="mono version">{plugin.version}</span>
                        {#if plugin.verified}
                          <span class="badge ok">verified</span>
                        {:else}
                          <span class="badge warn">unverified</span>
                        {/if}
                        <span class="badge" class:ok={plugin.enabled} class:warn={!plugin.enabled}>
                          {plugin.enabled ? "enabled" : "disabled"}
                        </span>
                      </div>
                      {#if plugin.theme}
                        <span class="swatches" aria-hidden="true">
                          <span style:background={plugin.theme.tokens.bg}></span>
                          <span style:background={plugin.theme.tokens.surface}></span>
                          <span style:background={plugin.theme.tokens.accent}></span>
                          <span style:background={plugin.theme.tokens.text}></span>
                        </span>
                      {/if}
                    </div>

                    <p class="installed-summary">{plugin.summary}</p>
                    <p class="fingerprint mono">{grouped(plugin.fingerprint)}</p>
                    {#if !plugin.verified}
                      <p class="state warn">
                        This record no longer passes signature verification. It is not applied;
                        reinstall it from its registry.
                      </p>
                    {/if}

                    <div class="installed-actions">
                      <button
                        class="btn ghost"
                        onclick={() => void toggle(plugin)}
                        disabled={plugins.busyId === plugin.id}
                      >
                        {plugin.enabled ? "Disable" : "Enable"}
                      </button>
                      <button
                        class="btn ghost danger"
                        onclick={() => void removePlugin(plugin)}
                        disabled={plugins.busyId === plugin.id}
                      >
                        Remove
                      </button>
                    </div>
                  </article>
                {/each}
              {/if}
            </div>
          {/each}
        {/if}
        {#if plugins.installError}
          <p class="state danger" role="alert">{plugins.installError}</p>
        {/if}
      </section>

      <section class="panel section">
        <div class="section-head">
          <span class="mono-label">trusted keys</span>
          <span class="badge">{plugins.keys.length}</span>
        </div>
        {#if plugins.keys.length === 0}
          <p class="empty-copy">
            No signing keys are trusted yet. The first install from a key shows its
            fingerprint for explicit approval.
          </p>
        {:else}
          {#each plugins.keys as key (key.fingerprint)}
            <article class="key">
              <div class="key-head">
                <span class="strong">{key.label || "Unlabelled key"}</span>
                <span class="badge" class:accent={key.usedBy.length > 0}>
                  {key.usedBy.length} plugin{key.usedBy.length === 1 ? "" : "s"}
                </span>
              </div>
              <p class="fingerprint mono">{grouped(key.fingerprint)}</p>
              {#if key.usedBy.length > 0}
                <p class="state faint">Used by {key.usedBy.join(", ")}.</p>
              {/if}
              <button
                class="btn ghost danger"
                onclick={() => void removeTrustedKey(key.fingerprint, key.label)}
                disabled={key.usedBy.length > 0}
                title={key.usedBy.length > 0 ? "Uninstall the plugins using this key first." : undefined}
              >
                Remove key
              </button>
            </article>
          {/each}
        {/if}
      </section>
    </div>

    <div class="column">
      <section class="panel section browse">
        <div class="section-head">
          <span class="mono-label">browse</span>
          {#if plugins.registry}
            <span class="badge">{registryCount}</span>
          {/if}
        </div>

        {#if plugins.registryLoading}
          <p class="state mono faint">fetching registry…</p>
        {:else if plugins.registryError}
          <p class="state faint">
            The registry could not be read. Installed plugins above remain available offline.
          </p>
        {:else if !plugins.registry}
          <div class="empty">
            <p class="empty-title">No registry loaded</p>
            <p class="empty-copy">
              GRAFT does not ship a catalogue of invented plugins. Point it at a registry
              you trust, or install nothing and keep the built-in experience.
            </p>
          </div>
        {:else if plugins.registry.plugins.length === 0}
          <p class="empty-copy">This registry lists no packages.</p>
        {:else}
          <div class="entry-list">
            {#each plugins.registry.plugins as entry (entry.id)}
              <button
                class="entry"
                class:active={selected?.id === entry.id}
                aria-pressed={selected?.id === entry.id}
                onclick={() => selectEntry(entry)}
              >
                <span class="entry-main">
                  <span class="strong">{entry.name}</span>
                  <span class="mono version">{entry.version}</span>
                  <span class="badge">{entry.kind}</span>
                </span>
                <span class="entry-summary">{entry.summary}</span>
                <span class="entry-id mono">{entry.id}</span>
              </button>
            {/each}
          </div>
        {/if}
      </section>

      {#if selected}
        <section class="panel section review">
          <div class="section-head">
            <span class="mono-label">review install</span>
            <span class="badge">{selected.kind}</span>
          </div>

          <h2 class="review-name">{selected.name}</h2>
          <p class="review-summary">{selected.summary}</p>

          <dl class="facts">
            <div>
              <dt>id</dt>
              <dd class="mono">{selected.id}</dd>
            </div>
            <div>
              <dt>version</dt>
              <dd class="mono">{selected.version}</dd>
            </div>
            <div>
              <dt>kind</dt>
              <dd class="mono">{selected.kind}</dd>
            </div>
            <div>
              <dt>capability</dt>
              <dd>{selectedCapability}</dd>
            </div>
          </dl>

          <div class="fingerprint-box">
            <span class="mono-label">signing key fingerprint</span>
            <p class="fingerprint mono">{grouped(selected.fingerprint)}</p>
            {#if keyTrusted}
              <p class="state ok">This key is already trusted on this machine.</p>
            {:else}
              <label class="trust">
                <input type="checkbox" bind:checked={trust} />
                <span>I trust this signing key</span>
              </label>
              <p class="state faint">
                A new key is never trusted silently. Compare this fingerprint with the
                publisher before ticking the box.
              </p>
            {/if}
          </div>

          <div class="review-actions">
            <button
              class="btn primary"
              disabled={!canInstall}
              onclick={() => void installSelected()}
            >
              {plugins.busyId === selected.id
                ? "Installing…"
                : selectedInstalled
                  ? "Reinstall / update"
                  : "Install"}
            </button>
            <button class="btn ghost" onclick={() => (selected = null)}>Cancel</button>
          </div>
          {#if plugins.installError}
            <p class="state danger" role="alert">{plugins.installError}</p>
          {/if}
        </section>
      {/if}

      <section class="panel section">
        <div class="section-head">
          <span class="mono-label">integrations</span>
          <span class="badge">{phaseTools.length} tools</span>
        </div>
        <p class="section-copy">
          Detection is local. Integration packages only add notes for tools GRAFT already
          probes; they never install, launch or configure anything.
        </p>

        {#if !session.tools.length}
          <p class="state mono faint">probing the environment…</p>
        {:else}
          <div class="tools">
            {#each phaseTools as tool (tool.id)}
              {@const info = toolMap.get(tool.id)}
              {@const notes = notesByTool.get(tool.id) ?? []}
              <article class="tool" class:found={!!info?.found}>
                <div class="tool-head">
                  <span class="strong">{tool.label}</span>
                  {#if info?.found}
                    <span class="badge ok">found</span>
                  {:else}
                    <span class="badge warn">not found</span>
                  {/if}
                  {#if info?.version}
                    <span class="mono tool-version">{info.version}</span>
                  {/if}
                </div>
                <p class="tool-role">{tool.role}</p>
                {#each notes as note}
                  <div class="note">
                    <span class="note-source mono">{note.plugin}</span>
                    <span>{note.notes}</span>
                    {#if note.docsUrl}
                      <button class="link" onclick={() => openDocs(note.docsUrl!)}>
                        {note.docsUrl}
                      </button>
                    {/if}
                  </div>
                {/each}
              </article>
            {/each}
          </div>
          {#if otherTools > 0}
            <p class="state faint">
              {otherTools} other detected tool{otherTools === 1 ? "" : "s"} in the Environment module.
            </p>
          {/if}
        {/if}
      </section>
    </div>
  </div>

  <p class="footer mono-label">Plugins are signed data. GRAFT never runs plugin code.</p>
</section>

<style>
  .plugins-view {
    max-width: 1500px;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .banner {
    padding: 8px 10px;
    margin-bottom: 10px;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
    font-size: 11px;
  }

  .layout {
    display: grid;
    grid-template-columns: 380px minmax(0, 1fr);
    gap: 12px;
    align-items: start;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .section {
    padding: 11px 12px 12px;
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-bottom: 8px;
    margin-bottom: 9px;
    border-bottom: 1px solid var(--border);
  }

  .section-copy {
    margin-bottom: 9px;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .registry-row {
    display: flex;
    gap: 6px;
  }

  .registry-row .field {
    min-width: 0;
  }

  .state {
    padding: 7px 0 2px;
    font-size: 11px;
    line-height: 1.5;
  }

  .state.danger {
    color: var(--danger);
  }

  .state.warn {
    color: var(--warn);
  }

  .state.ok {
    color: var(--ok);
  }

  .strong {
    color: var(--text);
    font-weight: 600;
  }

  .version {
    font-size: 10px;
    color: var(--text-faint);
  }

  .schema {
    margin-top: 9px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .schema summary {
    cursor: pointer;
    user-select: none;
  }

  .schema summary:hover {
    color: var(--text-muted);
  }

  .schema-copy {
    margin: 7px 0;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .schema pre {
    max-height: 210px;
    margin: 0;
    padding: 8px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.5;
  }

  .group + .group {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 5px;
  }

  .count {
    font-size: 9px;
    color: var(--text-faint);
  }

  .installed {
    padding: 8px;
    margin-top: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .installed.off {
    opacity: 0.72;
  }

  .installed-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }

  .installed-name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .installed-summary {
    margin-top: 4px;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-muted);
  }

  .swatches {
    display: flex;
    flex: 0 0 auto;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: var(--surface);
  }

  .swatches span {
    width: 10px;
    height: 10px;
    border-radius: 1px;
  }

  .fingerprint {
    margin-top: 5px;
    font-size: 9px;
    line-height: 1.5;
    letter-spacing: 0.02em;
    color: var(--text-faint);
    overflow-wrap: anywhere;
  }

  .installed-actions {
    display: flex;
    gap: 5px;
    margin-top: 3px;
  }

  .btn.ghost.danger {
    color: var(--danger);
  }

  .btn.ghost.danger:hover:not(:disabled) {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }

  .key {
    padding: 8px;
    margin-top: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .key-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .empty {
    padding: 12px 4px;
  }

  .empty-title {
    margin-bottom: 3px;
    font-size: 12px;
    font-weight: 600;
  }

  .empty-copy {
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .entry-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-height: 430px;
    overflow: auto;
  }

  .entry {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .entry:hover {
    border-color: var(--border-strong);
    background: var(--surface-alt);
  }

  .entry.active {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    background: var(--accent-soft);
  }

  .entry-main {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }

  .entry-summary {
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-muted);
  }

  .entry-id {
    font-size: 9px;
    color: var(--text-faint);
  }

  .review-name {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.3;
  }

  .review-summary {
    margin-top: 3px;
    font-size: 11px;
    line-height: 1.55;
    color: var(--text-muted);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 7px;
    margin: 10px 0;
  }

  .facts dt {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .facts dd {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .fingerprint-box {
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .trust {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 11px;
    color: var(--text);
    cursor: pointer;
  }

  .trust input {
    accent-color: var(--accent);
  }

  .review-actions {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }

  .tools {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .tool {
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .tool.found {
    border-color: color-mix(in srgb, var(--ok) 28%, var(--border));
  }

  .tool-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }

  .tool-version {
    font-size: 9px;
    color: var(--text-faint);
  }

  .tool-role {
    margin-top: 2px;
    font-size: 10px;
    color: var(--text-faint);
  }

  .note {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px;
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-muted);
  }

  .note-source {
    color: var(--accent);
    font-size: 9px;
  }

  .link {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--info);
    font: inherit;
    font-size: 10px;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    overflow-wrap: anywhere;
  }

  .footer {
    margin-top: 13px;
    text-align: center;
  }

  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .tools {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
