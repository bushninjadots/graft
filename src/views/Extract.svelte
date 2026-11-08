<script lang="ts">
  import { onMount } from "svelte";
  import type { ExtractComponent } from "../lib/api";
  import { plural, relativeTime } from "../lib/format";
  import {
    analyzeSelection,
    applySuggestion,
    buildPlan,
    clearSelection,
    executePlan,
    extract,
    isCovered,
    isSelected,
    invalidatePlan,
    loadRuns,
    resetSource,
    scanSource,
    setStrategy,
    toggleSelection,
    type ExtractStrategy,
  } from "../lib/state/extract.svelte";
  import Icon from "../lib/components/Icon.svelte";

  type BrowseEntry = {
    name: string;
    path: string;
    dir: boolean;
    files: number;
    bytes: number;
  };

  const strategies: { id: ExtractStrategy; label: string; hint: string }[] = [
    {
      id: "copy",
      label: "Copy",
      hint: "Write the selection into a new folder. The source repository is left untouched.",
    },
    {
      id: "move",
      label: "Move",
      hint: "Copy and verify first, store a backup, then remove the originals from the source.",
    },
    {
      id: "history",
      label: "With history",
      hint: "Clone the repository and use git subtree to carry the component directory's commits into a new repository.",
    },
  ];

  let browsePath = $state("");
  let confirming = $state(false);

  const scan = $derived(extract.scan);
  const plan = $derived(extract.plan);
  const analysis = $derived(extract.analysis);
  const failedChecks = $derived(plan ? plan.checks.filter((check) => !check.ok).length : 0);
  const writeCount = $derived(plan ? plan.actions.filter((action) => action.kind === "write").length : 0);
  const sideActions = $derived(
    plan ? plan.actions.filter((action) => action.kind !== "write") : [],
  );
  const selectedBytes = $derived.by(() => {
    if (!scan) return 0;
    const prefixes = extract.selection;
    return scan.files
      .filter((file) =>
        prefixes.some((entry) => file.path === entry || file.path.startsWith(`${entry}/`)),
      )
      .reduce((total, file) => total + file.bytes, 0);
  });

  const crumbs = $derived(browsePath ? browsePath.split("/") : []);

  const entries = $derived.by<BrowseEntry[]>(() => {
    if (!scan) return [];
    const prefix = browsePath ? `${browsePath}/` : "";
    const found = new Map<string, BrowseEntry>();
    for (const file of scan.files) {
      if (!file.path.startsWith(prefix)) continue;
      const rest = file.path.slice(prefix.length);
      if (!rest) continue;
      const slash = rest.indexOf("/");
      if (slash === -1) {
        found.set(rest, {
          name: rest,
          path: `${prefix}${rest}`,
          dir: false,
          files: 1,
          bytes: file.bytes,
        });
        continue;
      }
      const name = rest.slice(0, slash);
      const path = `${prefix}${name}`;
      const entry = found.get(name) ?? { name, path, dir: true, files: 0, bytes: 0 };
      entry.files += 1;
      entry.bytes += file.bytes;
      found.set(name, entry);
    }
    return [...found.values()].sort(
      (left, right) => Number(right.dir) - Number(left.dir) || left.name.localeCompare(right.name),
    );
  });

  const visibleSelection = $derived(extract.selection.slice(0, 12));
  const hiddenSelection = $derived(Math.max(0, extract.selection.length - visibleSelection.length));

  const strategy = $derived(strategies.find((entry) => entry.id === extract.strategy)!);
  const needsConfirmation = $derived(extract.strategy !== "copy");

  onMount(() => {
    void loadRuns();
  });

  $effect(() => {
    if (!plan) confirming = false;
  });

  function bytes(count: number): string {
    if (count < 1024) return `${count} B`;
    if (count < 1024 * 1024) return `${(count / 1024).toFixed(1)} kB`;
    if (count < 1024 * 1024 * 1024) return `${(count / (1024 * 1024)).toFixed(1)} MB`;
    return `${(count / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function percent(value: number): string {
    return `${Math.round(value * 100)}%`;
  }

  function enter(directory: string): void {
    browsePath = directory;
  }

  function up(): void {
    const parts = browsePath.split("/");
    parts.pop();
    browsePath = parts.join("/");
  }

  function selectCurrent(): void {
    if (!browsePath) return;
    if (isSelected(browsePath)) return;
    toggleSelection(browsePath);
  }

  function sourceChanged(): void {
    if (extract.scan && extract.source.trim() !== extract.scan.source) resetSource();
  }

  async function handleScan(): Promise<void> {
    browsePath = "";
    confirming = false;
    await scanSource();
  }

  function suggestionLabel(component: ExtractComponent): string {
    return component.path || "(repository root)";
  }

  async function runPlan(): Promise<boolean> {
    return buildPlan();
  }

  async function confirmExecute(): Promise<void> {
    confirming = false;
    await executePlan();
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Extract Studio</h1>
      <p class="view-sub">
        Find one component inside a large repository, understand what it depends on and what
        depends on it, then copy, move or split it into a new project with a plan you approve
        before anything is written.
      </p>
    </div>
    <div class="head-actions">
      <button class="btn ghost" onclick={() => void loadRuns()} disabled={extract.loadingRuns}>
        <span class:spin={extract.loadingRuns}><Icon name="refresh" size={12} /></span>
        {extract.loadingRuns ? "Refreshing…" : "Refresh runs"}
      </button>
      {#if extract.runs.length > 0}
        <span class="mono faint">{plural(extract.runs.length, "recorded run")}</span>
      {/if}
    </div>
  </div>

  <div class="panel source-panel">
    <div class="source-field">
      <span class="mono-label">repository</span>
      <div class="source-row">
        <input
          class="field"
          bind:value={extract.source}
          oninput={sourceChanged}
          onkeydown={(event) => {
            if (event.key === "Enter") void handleScan();
          }}
          placeholder="/home/you/Projects/large-repository"
          spellcheck="false"
          aria-label="Repository path to scan"
        />
        <button class="btn primary" onclick={() => void handleScan()} disabled={extract.scanning}>
          <span class:spin={extract.scanning}><Icon name="search" size={12} /></span>
          {extract.scanning ? "Scanning…" : "Scan repository"}
        </button>
      </div>
    </div>
    <p class="source-note">
      Scanning is read-only. GRAFT skips VCS metadata, dependency caches and build output, and
      never follows symlinks.
    </p>
  </div>

  {#if extract.error}
    <div class="panel notice">
      <p class="tone-danger">{extract.error}</p>
      {#if extract.scan}
        <button class="btn" onclick={() => void handleScan()}>Retry scan</button>
      {/if}
    </div>
  {/if}

  {#if !scan}
    <div class="panel empty">
      <Icon name="extract" size={26} />
      <p class="empty-title">Point Extract Studio at a repository</p>
      <p class="empty-copy">
        After a scan you can inspect languages and manifests, choose a folder or file, and see
        its internal references, incoming coupling and external dependencies before planning an
        extraction.
      </p>
    </div>
  {:else}
    <div class="panel summary-panel">
      <div class="summary-main">
        <span class="path mono">{scan.source}</span>
        <div class="facts mono">
          <span>{plural(scan.totalFiles, "file")}</span>
          <span>{bytes(scan.totalBytes)}</span>
          <span>{scan.totalLines.toLocaleString()} lines</span>
          <span class:branch-ok={scan.isGitRepo}>
            {scan.isGitRepo ? scan.branch ?? "detached HEAD" : "not a git repository"}
          </span>
          {#if scan.truncated}<span class="tone-warn">scan truncated</span>{/if}
        </div>
      </div>
      <div class="language-list">
        {#each scan.languages as language (language.language)}
          <span class="badge">
            {language.language}
            <span class="faint">{language.files}</span>
          </span>
        {/each}
      </div>
      {#if scan.markers.length > 0}
        <div class="markers">
          <span class="mono-label">manifests</span>
          {#each scan.markers as marker (marker)}
            <span class="badge accent">{marker}</span>
          {/each}
        </div>
      {/if}
      {#if scan.warnings.length > 0}
        <ul class="warnings mono">
          {#each scan.warnings as warning (warning)}
            <li>{warning}</li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="studio-grid">
      <div class="panel suggestion-panel">
        <div class="cell-head">
          <div>
            <span class="mono-label">suggested components</span>
            <p class="cell-sub">Directories with enough files and internal cohesion to stand alone.</p>
          </div>
          {#if extract.suggesting}<span class="mono faint">scoring…</span>{/if}
        </div>
        {#if extract.suggesting && extract.suggestions.length === 0}
          <p class="state mono">reading the import graph…</p>
        {:else if extract.suggestions.length === 0}
          <p class="state">No directory scored high enough. Pick a folder from the browser.</p>
        {:else}
          <div class="suggestion-list">
            {#each extract.suggestions as component (component.path)}
              <button
                class="suggestion"
                class:active={isSelected(component.path) || isCovered(component.path)}
                onclick={() => applySuggestion(component)}
                title={`Select ${suggestionLabel(component)}`}
              >
                <span class="suggestion-top">
                  <span class="suggestion-name mono">{suggestionLabel(component)}</span>
                  {#if component.hasManifest}<span class="badge accent">manifest</span>{/if}
                  <span class="suggestion-score mono">{component.score.toFixed(1)}</span>
                </span>
                <span class="suggestion-facts mono">
                  <span>{plural(component.files, "file")}</span>
                  <span>{bytes(component.bytes)}</span>
                  <span>cohesion {percent(component.cohesion)}</span>
                  <span>in {component.incomingReferences}</span>
                  <span>out {component.outgoingReferences}</span>
                  <span>{plural(component.externalDeps, "package")}</span>
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="panel browser-panel">
        <div class="cell-head">
          <div>
            <span class="mono-label">selection</span>
            <p class="cell-sub">Add folders or individual files. Folders include everything below them.</p>
          </div>
          {#if extract.selection.length > 0}
            <button class="btn ghost" onclick={clearSelection}>Clear</button>
          {/if}
        </div>

        <div class="crumbs">
          <button class="crumb" onclick={() => enter("")} class:current={!browsePath}>root</button>
          {#each crumbs as crumb, index (index)}
            <Icon name="chevron-right" size={11} />
            <button
              class="crumb"
              class:current={index === crumbs.length - 1}
              onclick={() => enter(crumbs.slice(0, index + 1).join("/"))}
            >{crumb}</button>
          {/each}
          {#if browsePath}
            <button class="btn ghost up" onclick={up} title="Go up one folder">↑</button>
            <button class="btn ghost up" onclick={selectCurrent} disabled={isSelected(browsePath)}>
              Select this folder
            </button>
          {/if}
        </div>

        <div class="entry-list">
          {#if entries.length === 0}
            <p class="state">This folder contains no scanned files.</p>
          {:else}
            {#each entries as entry (entry.path)}
              <div class="entry" class:covered={isCovered(entry.path) && !isSelected(entry.path)}>
                <button
                  class="check"
                  class:on={isCovered(entry.path)}
                  onclick={() => toggleSelection(entry.path)}
                  disabled={isCovered(entry.path) && !isSelected(entry.path)}
                  aria-label={`Select ${entry.path}`}
                  title={isCovered(entry.path) && !isSelected(entry.path) ? "Included by a selected parent folder" : "Select"}
                >
                  {#if isSelected(entry.path)}<Icon name="check" size={11} />{/if}
                </button>
                {#if entry.dir}
                  <button class="entry-name mono" onclick={() => enter(entry.path)}>
                    <Icon name="folder-git" size={12} />
                    <span>{entry.name}</span>
                    <Icon name="chevron-right" size={11} />
                  </button>
                {:else}
                  <span class="entry-name mono">
                    <Icon name="dot" size={11} />
                    <span>{entry.name}</span>
                  </span>
                {/if}
                <span class="entry-meta mono faint">
                  {plural(entry.files, "file")} · {bytes(entry.bytes)}
                </span>
              </div>
            {/each}
          {/if}
        </div>

        {#if extract.selection.length > 0}
          <div class="selected">
            <div class="selected-head">
              <span class="mono-label">selected</span>
              <span class="mono faint">{plural(extract.selection.length, "path")} · {bytes(selectedBytes)}</span>
            </div>
            <div class="chips">
              {#each visibleSelection as entry (entry)}
                <button class="chip mono" onclick={() => toggleSelection(entry)} title="Remove from selection">
                  {entry}
                  <Icon name="x" size={10} />
                </button>
              {/each}
              {#if hiddenSelection > 0}
                <span class="chip mono">+{hiddenSelection} more</span>
              {/if}
            </div>
            <button class="btn" onclick={() => void analyzeSelection()} disabled={extract.analyzing}>
              <span class:spin={extract.analyzing}><Icon name="search" size={12} /></span>
              {extract.analyzing ? "Analyzing…" : "Analyze selection"}
            </button>
          </div>
        {/if}
      </div>
    </div>

    {#if analysis}
      <div class="panel analysis-panel">
        <div class="cell-head">
          <div>
            <span class="mono-label">coupling analysis</span>
            <p class="cell-sub">
              {plural(analysis.fileCount, "file")} · {bytes(analysis.totalBytes)} ·
              {analysis.totalLines.toLocaleString()} lines
            </p>
          </div>
          <div class="head-actions">
            <span class="badge" class:accent={analysis.cohesion >= 0.5}>
              cohesion {percent(analysis.cohesion)}
            </span>
            <span class="badge" class:ok={analysis.incomingReferences === 0}>
              {plural(analysis.incomingReferences, "incoming reference")}
            </span>
          </div>
        </div>

        <div class="metric-grid">
          <div class="metric">
            <span class="metric-value mono">{analysis.internalReferences}</span>
            <span class="mono-label">internal edges</span>
          </div>
          <div class="metric">
            <span class="metric-value mono">{analysis.outgoingReferences}</span>
            <span class="mono-label">outgoing</span>
          </div>
          <div class="metric">
            <span class="metric-value mono">{analysis.incomingReferences}</span>
            <span class="mono-label">incoming</span>
          </div>
          <div class="metric">
            <span class="metric-value mono">{analysis.externalDeps.length}</span>
            <span class="mono-label">external packages</span>
          </div>
        </div>

        <div class="analysis-columns">
          <div>
            <div class="subsection-head">
              <span class="mono-label">external dependencies</span>
              <span class="mono faint">{analysis.externalDeps.length}</span>
            </div>
            {#if analysis.externalDeps.length === 0}
              <p class="state">No external packages appear in the import graph.</p>
            {:else}
              <div class="chips">
                {#each analysis.externalDeps as dep (dep.name)}
                  <span class="chip mono">{dep.name} <span class="faint">{dep.files}</span></span>
                {/each}
              </div>
            {/if}

            <div class="subsection-head">
              <span class="mono-label">files</span>
              <span class="mono faint">{analysis.files.length} shown</span>
            </div>
            <div class="file-list mono">
              {#each analysis.files as file (file.path)}
                <span class="file-row">
                  <span>{file.path}</span>
                  <span class="faint">{file.language} · {file.lines} lines</span>
                </span>
              {/each}
            </div>
          </div>

          <div>
            <div class="subsection-head">
              <span class="mono-label">incoming references</span>
              <span class="mono faint">{analysis.incoming.length} shown</span>
            </div>
            {#if analysis.incoming.length === 0}
              <p class="state">Nothing outside the selection imports it. That is a clean cut.</p>
            {:else}
              <div class="ref-list mono">
                {#each analysis.incoming as ref (ref.from + ref.to + ref.line)}
                  <span class="ref-row">
                    <span class="ref-from">{ref.from}:{ref.line}</span>
                    <span class="faint">→</span>
                    <span class="ref-to">{ref.to}</span>
                  </span>
                {/each}
              </div>
            {/if}

            {#if analysis.warnings.length > 0}
              <div class="subsection-head">
                <span class="mono-label">warnings</span>
              </div>
              <ul class="warnings mono">
                {#each analysis.warnings as warning (warning)}
                  <li>{warning}</li>
                {/each}
              </ul>
            {/if}
            {#if analysis.scanWarnings > 0}
              <p class="state mono">{plural(analysis.scanWarnings, "scan warning")} carried over.</p>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    <div class="panel plan-panel">
      <div class="cell-head">
        <div>
          <span class="mono-label">extraction plan</span>
          <p class="cell-sub">Nothing is written until the plan passes its checks and you confirm.</p>
        </div>
        {#if plan}
          <span class="badge" class:ok={failedChecks === 0} class:warn={failedChecks > 0}>
            {failedChecks === 0 ? "ready" : plural(failedChecks, "failed check")}
          </span>
        {/if}
      </div>

      <div class="strategy-row">
        {#each strategies as option (option.id)}
          <button
            class="strategy"
            class:active={extract.strategy === option.id}
            onclick={() => setStrategy(option.id)}
          >
            <span class="strategy-label">{option.label}</span>
            <span class="strategy-hint">{option.hint}</span>
          </button>
        {/each}
      </div>

      <div class="plan-fields">
        <label class="plan-field">
          <span class="mono-label">new folder</span>
          <input class="field" bind:value={extract.target} oninput={invalidatePlan} placeholder="~/Projects/my-component" spellcheck="false" />
        </label>
        <label class="plan-field">
          <span class="mono-label">project name</span>
          <input class="field" bind:value={extract.name} oninput={invalidatePlan} placeholder="my-component" spellcheck="false" />
        </label>
        <button
          class="btn primary plan-button"
          onclick={() => void runPlan()}
          disabled={extract.planning || extract.selection.length === 0}
        >
          <span class:spin={extract.planning}><Icon name="check" size={12} /></span>
          {extract.planning ? "Checking…" : "Build plan"}
        </button>
      </div>
      <p class="strategy-note">{strategy.hint}</p>

      {#if plan}
        <div class="plan-body">
          <div class="plan-facts mono">
            <span>{plural(plan.fileCount, "file")}</span>
            <span>{bytes(plan.totalBytes)}</span>
            <span>{plan.strategy}</span>
            <span class="faint">→</span>
            <span>{plan.target}</span>
          </div>

          <div class="check-list">
            {#each plan.checks as check (check.id)}
              <div class="check-row" class:bad={!check.ok}>
                <span class="check-icon">
                  <Icon name={check.ok ? "check" : "x"} size={11} />
                </span>
                <span class="check-label">{check.label}</span>
                <span class="check-detail mono faint">{check.detail}</span>
              </div>
            {/each}
          </div>

          {#if plan.warnings.length > 0}
            <ul class="warnings mono">
              {#each plan.warnings as warning (warning)}
                <li>{warning}</li>
              {/each}
            </ul>
          {/if}

          <div class="action-summary">
            <span class="mono-label">writes</span>
            <span class="mono faint">{plural(writeCount, "file")} into {plan.target}</span>
            {#if plan.requiresBackup && plan.backupPath}
              <span class="mono-label backup-label">backup</span>
              <span class="mono faint">{plan.backupPath}</span>
            {/if}
            {#if plan.historyPrefix}
              <span class="mono-label backup-label">history prefix</span>
              <span class="mono faint">{plan.historyPrefix}/</span>
            {/if}
          </div>
          {#if sideActions.length > 0}
            <div class="side-actions">
              {#each sideActions as action (action.kind + action.path)}
                <div class="side-action mono">
                  <span class="badge">{action.kind}</span>
                  <span>{action.detail}</span>
                </div>
              {/each}
            </div>
          {/if}

          {#if confirming}
            <div class="confirm">
              <p>
                {#if plan.strategy === "move"}
                  Move will copy {plural(plan.fileCount, "file")} to {plan.target}, verify them,
                  write a full backup, then remove the originals from {plan.source}. The source
                  repository will change.
                {:else}
                  Git history extraction clones {plan.source}, rewrites the component directory
                  into a standalone repository at {plan.target}, and keeps the source untouched.
                {/if}
              </p>
              <div class="confirm-actions">
                <button class="btn ghost" onclick={() => (confirming = false)} disabled={extract.executing}>
                  Cancel
                </button>
                <button class="btn primary" onclick={() => void confirmExecute()} disabled={extract.executing}>
                  {#if extract.executing}
                    <span class="spin"><Icon name="refresh" size={12} /></span>
                    Working…
                  {:else}
                    {plan.strategy === "move" ? "Move and remove originals" : "Split history"}
                  {/if}
                </button>
              </div>
            </div>
          {:else}
            <div class="execute-row">
              <button
                class="btn"
                class:primary={!needsConfirmation}
                onclick={() => (needsConfirmation ? (confirming = true) : void confirmExecute())}
                disabled={failedChecks > 0 || extract.executing}
              >
                {#if extract.executing}
                  <span class="spin"><Icon name="refresh" size={12} /></span>
                  {extract.strategy === "history" ? "Splitting history…" : "Writing…"}
                {:else if plan.strategy === "copy"}
                  Copy to new project
                {:else if plan.strategy === "move"}
                  Review move…
                {:else}
                  Review history extraction…
                {/if}
              </button>
              <span class="mono faint">
                {failedChecks > 0 ? "Resolve failed checks first." : "Pre-flight checks passed."}
              </span>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    {#if extract.result}
      <div class="panel result-panel">
        <div class="cell-head">
          <div class="result-title">
            <Icon name="check" size={14} />
            <span>Extraction complete</span>
          </div>
          <span class="badge ok">{extract.result.strategy}</span>
        </div>
        <p class="result-message">{extract.result.message}</p>
        <div class="result-facts mono">
          <span>{plural(extract.result.fileCount, "file")}</span>
          <span>{bytes(extract.result.totalBytes)}</span>
          <span>{extract.result.target}</span>
        </div>
        {#if extract.result.backupPath}
          <p class="mono faint">backup: {extract.result.backupPath}</p>
        {/if}
        <div class="result-checks">
          {#each extract.result.checks as check (check.id)}
            <span class="badge" class:ok={check.ok} class:warn={!check.ok}>{check.label}</span>
          {/each}
        </div>
      </div>
    {/if}
  {/if}

  <div class="panel runs-panel">
    <div class="cell-head">
      <div>
        <span class="mono-label">recent extractions</span>
        <p class="cell-sub">Recorded after a successful write, with its backup and plan outcome.</p>
      </div>
      <span class="mono faint">{plural(extract.runs.length, "run")}</span>
    </div>
    {#if extract.loadingRuns && extract.runs.length === 0}
      <p class="state mono">reading extraction history…</p>
    {:else if extract.runs.length === 0}
      <p class="state">No extraction has run on this machine yet.</p>
    {:else}
      <div class="run-list">
        {#each extract.runs as run (run.id)}
          <div class="run-row">
            <span class="run-name mono">{run.name}</span>
            <span class="badge">{run.strategy}</span>
            <span class="run-target mono faint">{run.source} → {run.target}</span>
            <span class="run-meta mono faint">
              {plural(run.fileCount, "file")} · {bytes(run.totalBytes)} · {relativeTime(run.createdAt)}
            </span>
          </div>
          {#if run.backupPath}
            <p class="run-backup mono faint">backup: {run.backupPath}</p>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</section>

<style>
  .head-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .source-panel {
    padding: 13px 14px;
    display: grid;
    gap: 7px;
    margin-bottom: 12px;
  }

  .source-field {
    display: grid;
    gap: 5px;
  }

  .source-row {
    display: flex;
    gap: 8px;
  }

  .source-row .field {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--font-small);
  }

  .source-note {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-small);
  }

  .notice {
    padding: 11px 13px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .notice p {
    margin: 0;
  }

  .empty {
    padding: 34px 22px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 7px;
    color: var(--text-muted);
    margin-bottom: 12px;
  }

  .empty-title {
    margin: 4px 0 0;
    color: var(--text);
    font-weight: 600;
  }

  .empty-copy {
    margin: 0;
    max-width: 62ch;
    font-size: var(--font-small);
    color: var(--text-faint);
    line-height: 1.55;
  }

  .summary-panel {
    padding: 12px 14px;
    display: grid;
    gap: 9px;
    margin-bottom: 12px;
  }

  .summary-main {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .path {
    color: var(--text);
    font-size: var(--font-small);
    overflow-wrap: anywhere;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .branch-ok {
    color: var(--ok);
  }

  .language-list,
  .markers {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }

  .markers {
    gap: 6px;
  }

  .studio-grid {
    display: grid;
    grid-template-columns: minmax(280px, 0.9fr) minmax(340px, 1.1fr);
    gap: 12px;
    margin-bottom: 12px;
    align-items: start;
  }

  @media (max-width: 980px) {
    .studio-grid {
      grid-template-columns: 1fr;
    }
  }

  .suggestion-panel,
  .browser-panel,
  .analysis-panel,
  .plan-panel,
  .result-panel,
  .runs-panel {
    padding: 12px 13px;
  }

  .cell-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 9px;
  }

  .cell-sub {
    margin: 3px 0 0;
    color: var(--text-faint);
    font-size: var(--font-small);
    line-height: 1.45;
  }

  .state {
    margin: 8px 0;
    color: var(--text-muted);
    font-size: var(--font-small);
  }

  .suggestion-list {
    display: grid;
    gap: 5px;
    max-height: 430px;
    overflow-y: auto;
  }

  .suggestion {
    display: grid;
    gap: 5px;
    text-align: left;
    padding: 8px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text);
    cursor: pointer;
    font: inherit;
  }

  .suggestion:hover {
    border-color: var(--border-strong);
  }

  .suggestion.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .suggestion-top {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .suggestion-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--font-small);
  }

  .suggestion-score {
    color: var(--accent);
    font-size: var(--font-small);
  }

  .suggestion-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 9px;
    color: var(--text-faint);
    font-size: 11px;
  }

  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-bottom: 8px;
    color: var(--text-faint);
  }

  .crumb {
    border: 0;
    background: none;
    padding: 1px 3px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
    cursor: pointer;
  }

  .crumb:hover {
    color: var(--text);
  }

  .crumb.current {
    color: var(--accent);
  }

  .up {
    margin-left: 3px;
    font-size: 11px;
  }

  .entry-list {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    max-height: 360px;
    overflow-y: auto;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 29px;
    padding: 2px 7px;
    border-bottom: 1px solid var(--border);
  }

  .entry:last-child {
    border-bottom: 0;
  }

  .entry.covered {
    background: var(--accent-soft);
  }

  .check {
    width: 16px;
    height: 16px;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
    background: var(--bg-elev);
    color: var(--accent-ink);
    cursor: pointer;
  }

  .check.on {
    background: var(--accent);
    border-color: var(--accent);
  }

  .check:disabled {
    cursor: default;
  }

  .entry-name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 5px;
    border: 0;
    background: none;
    padding: 0;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.entry-name {
    cursor: pointer;
  }

  button.entry-name:hover span {
    color: var(--accent);
  }

  .entry-meta {
    flex: 0 0 auto;
    font-size: 10px;
  }

  .selected {
    margin-top: 9px;
    display: grid;
    gap: 6px;
  }

  .selected-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 7px;
    background: var(--surface-alt);
    color: var(--text-muted);
    font-size: 10px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button.chip {
    cursor: pointer;
  }

  button.chip:hover {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .analysis-panel,
  .plan-panel,
  .result-panel {
    margin-bottom: 12px;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
    margin-bottom: 11px;
  }

  @media (max-width: 720px) {
    .metric-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .metric {
    display: grid;
    gap: 3px;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .metric-value {
    font-size: 15px;
    color: var(--text);
  }

  .analysis-columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  @media (max-width: 900px) {
    .analysis-columns {
      grid-template-columns: 1fr;
    }
  }

  .subsection-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 9px 0 5px;
  }

  .file-list,
  .ref-list {
    display: grid;
    gap: 2px;
    max-height: 220px;
    overflow-y: auto;
  }

  .file-row,
  .ref-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
    padding: 2px 5px;
    border-radius: 3px;
    font-size: 10px;
  }

  .file-row:nth-child(odd),
  .ref-row:nth-child(odd) {
    background: var(--surface-alt);
  }

  .file-row span:first-child,
  .ref-from {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ref-row {
    justify-content: flex-start;
  }

  .ref-to {
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .warnings {
    margin: 7px 0 0;
    padding: 0 0 0 15px;
    display: grid;
    gap: 3px;
    color: var(--warn);
    font-size: 10px;
    line-height: 1.45;
  }

  .strategy-row {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
    margin-bottom: 10px;
  }

  @media (max-width: 760px) {
    .strategy-row {
      grid-template-columns: 1fr;
    }
  }

  .strategy {
    display: grid;
    gap: 3px;
    text-align: left;
    padding: 8px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text);
    cursor: pointer;
    font: inherit;
  }

  .strategy:hover {
    border-color: var(--border-strong);
  }

  .strategy.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .strategy-label {
    font-weight: 600;
    font-size: var(--font-small);
  }

  .strategy-hint {
    color: var(--text-faint);
    font-size: 10px;
    line-height: 1.4;
  }

  .plan-fields {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    flex-wrap: wrap;
  }

  .plan-field {
    display: grid;
    gap: 4px;
    flex: 1;
    min-width: 190px;
  }

  .plan-field .field {
    font-family: var(--font-mono);
    font-size: var(--font-small);
  }

  .plan-button {
    height: calc(var(--row-h) + 4px);
  }

  .strategy-note {
    margin: 7px 0 0;
    color: var(--text-faint);
    font-size: 10px;
  }

  .plan-body {
    margin-top: 11px;
    border-top: 1px solid var(--border);
    padding-top: 10px;
    display: grid;
    gap: 9px;
  }

  .plan-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .check-list {
    display: grid;
    gap: 3px;
  }

  .check-row {
    display: flex;
    align-items: baseline;
    gap: 7px;
    font-size: var(--font-small);
    padding: 3px 5px;
    border-radius: 3px;
    background: var(--surface-alt);
  }

  .check-row.bad {
    background: transparent;
    border: 1px solid var(--danger);
  }

  .check-icon {
    color: var(--ok);
    display: grid;
    place-items: center;
  }

  .check-row.bad .check-icon {
    color: var(--danger);
  }

  .check-label {
    flex: 0 0 auto;
    color: var(--text);
  }

  .check-detail {
    flex: 1;
    min-width: 0;
    text-align: right;
    overflow-wrap: anywhere;
    font-size: 10px;
  }

  .action-summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 7px;
  }

  .backup-label {
    margin-left: 6px;
  }

  .side-actions {
    display: grid;
    gap: 4px;
  }

  .side-action {
    display: flex;
    align-items: baseline;
    gap: 7px;
    font-size: 10px;
    color: var(--text-muted);
  }

  .execute-row {
    display: flex;
    align-items: center;
    gap: 9px;
    flex-wrap: wrap;
  }

  .confirm {
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    padding: 10px 11px;
    display: grid;
    gap: 9px;
    background: var(--bg-elev);
  }

  .confirm p {
    margin: 0;
    font-size: var(--font-small);
    line-height: 1.5;
    color: var(--text-muted);
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .result-panel {
    border-color: var(--ok);
  }

  .result-title {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--ok);
    font-weight: 600;
  }

  .result-message {
    margin: 0 0 7px;
    font-size: var(--font-small);
    line-height: 1.5;
  }

  .result-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .result-checks {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 7px;
  }

  .run-list {
    display: grid;
    gap: 3px;
  }

  .run-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
    padding: 5px 6px;
    border-radius: 3px;
    font-size: 11px;
  }

  .run-row:nth-child(odd) {
    background: var(--surface-alt);
  }

  .run-name {
    color: var(--text);
    font-weight: 600;
  }

  .run-target {
    flex: 1;
    min-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .run-backup {
    margin: 0 0 0 6px;
    font-size: 10px;
  }

  .spin {
    display: inline-flex;
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
