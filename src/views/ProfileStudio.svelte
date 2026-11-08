<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { api, type ProfileReadme } from "../lib/api";
  import { githubUser, isSignedIn } from "../lib/state/github.svelte";
  import { navigate, notify } from "../lib/state/ui.svelte";
  import { relativeTime } from "../lib/format";
  import Icon from "../lib/components/Icon.svelte";

  const DRAFT_KEY = "profile.draft";
  const DRAFT_SAVED_KEY = "profile.draftSavedAt";

  type Mode = "preview" | "diff";
  type DiffLine = { kind: "same" | "add" | "del"; text: string; oldNo: number | null; newNo: number | null };

  let draft = $state("");
  let savedAt = $state<number | null>(null);
  let saving = $state(false);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let remote = $state<ProfileReadme | null>(null);
  let remoteLoading = $state(false);
  let remoteError = $state<string | null>(null);

  let mode = $state<Mode>("preview");
  let publishOpen = $state(false);
  let commitMessage = $state("Update profile README");
  let publishing = $state(false);
  let publishError = $state<string | null>(null);
  let lastPublished = $state<string | null>(null);

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  const readonly = $derived(!isSignedIn());

  function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : String(cause);
  }

  function render(source: string): string {
    const parsed = marked.parse(source, { async: false, gfm: true });
    return DOMPurify.sanitize(typeof parsed === "string" ? parsed : "");
  }

  onMount(() => {
    void (async () => {
      try {
        draft = (await api.getSetting(DRAFT_KEY)) ?? "";
        const stored = Number(await api.getSetting(DRAFT_SAVED_KEY));
        savedAt = Number.isFinite(stored) && stored > 0 ? stored : null;
      } catch (cause) {
        loadError = message(cause);
      } finally {
        loading = false;
      }
      void loadRemote();
    })();
  });

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
  });

  async function loadRemote(): Promise<void> {
    const user = githubUser();
    if (!isSignedIn() || !user) {
      remote = null;
      return;
    }
    remoteLoading = true;
    remoteError = null;
    try {
      remote = await api.githubProfileReadme(user.login);
    } catch (cause) {
      remoteError = message(cause);
    } finally {
      remoteLoading = false;
    }
  }

  function scheduleSave(): void {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void saveDraft(true), 700);
  }

  async function saveDraft(silent = false): Promise<void> {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    saving = true;
    try {
      await api.setSetting(DRAFT_KEY, draft);
      savedAt = Date.now();
      await api.setSetting(DRAFT_SAVED_KEY, String(savedAt));
      if (!silent) notify("Draft saved locally", "ok");
    } catch (cause) {
      notify(message(cause), "danger");
    } finally {
      saving = false;
    }
  }

  function useRemote(): void {
    if (remote?.content == null) return;
    draft = remote.content;
    void saveDraft(true);
    notify("Live README copied into the draft", "ok");
  }

  function starter(): void {
    if (draft.trim()) return;
    const user = githubUser();
    draft = [
      `# ${user?.name ?? user?.login ?? "Hello"}`,
      "",
      "> One line about what you build.",
      "",
      "## Currently",
      "",
      "- Working on:",
      "- Learning:",
      "",
      "## Stack",
      "",
      "```text",
      "Rust · TypeScript · Svelte · Linux",
      "```",
      "",
      `Find me on [GitHub](https://github.com/${user?.login ?? ""}).`,
      "",
    ].join("\n");
    void saveDraft(true);
  }

  function openProfile(): void {
    const url = githubUser()?.htmlUrl;
    if (!url) return;
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

  /** Line diff against the live README, capped so huge files stay responsive. */
  function diffLines(before: string, after: string): DiffLine[] {
    const a = before.length ? before.split("\n") : [];
    const b = after.length ? after.split("\n") : [];
    const capped = a.length * b.length > 4_000_000;
    if (capped) {
      return [
        ...a.map((text, index) => ({ kind: "del" as const, text, oldNo: index + 1, newNo: null })),
        ...b.map((text, index) => ({ kind: "add" as const, text, oldNo: null, newNo: index + 1 })),
      ];
    }

    const table: Uint32Array[] = Array.from({ length: a.length + 1 }, () => new Uint32Array(b.length + 1));
    for (let i = a.length - 1; i >= 0; i--) {
      for (let j = b.length - 1; j >= 0; j--) {
        table[i][j] = a[i] === b[j]
          ? table[i + 1][j + 1] + 1
          : Math.max(table[i + 1][j], table[i][j + 1]);
      }
    }

    const out: DiffLine[] = [];
    let i = 0;
    let j = 0;
    while (i < a.length && j < b.length) {
      if (a[i] === b[j]) {
        out.push({ kind: "same", text: a[i], oldNo: i + 1, newNo: j + 1 });
        i++;
        j++;
      } else if (table[i + 1][j] >= table[i][j + 1]) {
        out.push({ kind: "del", text: a[i], oldNo: i + 1, newNo: null });
        i++;
      } else {
        out.push({ kind: "add", text: b[j], oldNo: null, newNo: j + 1 });
        j++;
      }
    }
    while (i < a.length) {
      out.push({ kind: "del", text: a[i], oldNo: i + 1, newNo: null });
      i++;
    }
    while (j < b.length) {
      out.push({ kind: "add", text: b[j], oldNo: null, newNo: j + 1 });
      j++;
    }
    return out;
  }

  const diff = $derived(diffLines(remote?.content ?? "", draft));
  const added = $derived(diff.filter((line) => line.kind === "add").length);
  const removed = $derived(diff.filter((line) => line.kind === "del").length);

  function confirmPublish(): void {
    if (!isSignedIn()) {
      notify("Sign in to GitHub before publishing.", "danger");
      return;
    }
    if (!draft.trim()) {
      notify("The draft is empty — write something first.", "danger");
      return;
    }
    publishError = null;
    publishOpen = true;
  }

  async function publish(): Promise<void> {
    const user = githubUser();
    if (!user || publishing) return;
    publishing = true;
    publishError = null;
    try {
      const result = await api.githubPublishProfile(
        user.login,
        draft,
        remote?.sha ?? null,
        commitMessage,
      );
      remote = {
        exists: true,
        content: draft,
        sha: result.sha,
        htmlUrl: result.htmlUrl,
      };
      lastPublished = result.htmlUrl ?? `https://github.com/${user.login}/${user.login}`;
      publishOpen = false;
      notify("Profile README published", "ok");
    } catch (cause) {
      publishError = message(cause);
    } finally {
      publishing = false;
    }
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Profile Studio</h1>
      <p class="view-sub">
        Write your GitHub profile README here. The draft stays in local SQLite; nothing
        reaches GitHub until you confirm a publish.
      </p>
    </div>
    <div class="head-status">
      {#if isSignedIn() && githubUser()}
        <span class="badge ok">@{githubUser()?.login}</span>
        <button class="btn ghost" onclick={openProfile}>Open profile ↗</button>
      {:else}
        <span class="badge warn">draft only</span>
      {/if}
    </div>
  </div>

  {#if loading}
    <p class="state mono">reading local draft…</p>
  {:else if loadError}
    <div class="panel notice">
      <p class="tone-danger">The draft could not be read.</p>
      <p class="faint">{loadError}</p>
    </div>
  {:else}
    {#if readonly}
      <div class="panel notice">
        <p>You can write and save the draft without an account, but publishing needs GitHub.</p>
        <button class="btn" onclick={() => navigate("repositories")}>Connect GitHub</button>
      </div>
    {/if}

    <div class="studio">
      <div class="panel editor">
        <div class="bar">
          <span class="mono-label">draft · markdown</span>
          <div class="bar-right">
            <span class="mono faint">
              {draft.split("\n").length} lines · {draft.length} chars
            </span>
            <button class="btn ghost" onclick={() => void saveDraft()} disabled={saving}>
              {saving ? "Saving…" : "Save draft"}
            </button>
          </div>
        </div>
        <textarea
          class="field code-area"
          bind:value={draft}
          oninput={scheduleSave}
          spellcheck="false"
          placeholder="# Hi, I'm …"
          aria-label="Profile README markdown"
        ></textarea>
        <div class="foot">
          <span class="mono faint">
            {#if saving}
              saving…
            {:else if savedAt}
              saved {relativeTime(Math.floor(savedAt / 1000))}
            {:else}
              not saved yet
            {/if}
          </span>
          <div class="row">
            {#if !draft.trim()}
              <button class="btn ghost" onclick={starter}>Starter template</button>
            {/if}
            {#if remote?.content != null && remote.content !== draft}
              <button class="btn ghost" onclick={useRemote}>Use live README</button>
            {/if}
            <button class="btn ghost" onclick={() => void loadRemote()} disabled={remoteLoading || !isSignedIn()}>
              {remoteLoading ? "Checking live…" : "Refresh live"}
            </button>
            <button class="btn primary" onclick={confirmPublish} disabled={!isSignedIn() || !draft.trim()}>
              <Icon name="arrow-up-right" size={12} /> Publish
            </button>
          </div>
        </div>
      </div>

      <div class="panel preview">
        <div class="bar">
          <div class="segmented">
            <button class="seg" class:active={mode === "preview"} onclick={() => (mode = "preview")}>Preview</button>
            <button class="seg" class:active={mode === "diff"} onclick={() => (mode = "diff")}>
              Diff {#if remote?.exists === false}<span class="faint">·</span>{/if}
            </button>
          </div>
          <span class="mono faint">
            {#if remoteLoading}
              reading live README…
            {:else if remote?.exists === false}
              no live README yet
            {:else if remote}
              live · {remote.sha?.slice(0, 7) ?? "current"}
            {:else}
              offline draft
            {/if}
          </span>
        </div>

        {#if remoteError}
          <p class="error">{remoteError}</p>
        {/if}

        {#if mode === "preview"}
          {#if draft.trim()}
            <div class="markdown">{@html render(draft)}</div>
          {:else}
            <p class="state">Nothing to preview yet. Write Markdown on the left, or start from the template.</p>
          {/if}
        {:else}
          <div class="diff-head">
            <span class="mono"><span class="add">+{added}</span> <span class="del">−{removed}</span></span>
            <span class="mono faint">
              {#if remote?.exists === false}
                publishing creates README.md in {githubUser()?.login}/{githubUser()?.login}
              {:else}
                live README → draft
              {/if}
            </span>
          </div>
          {#if diff.length === 0}
            <p class="state">The draft matches the live README.</p>
          {:else}
            <div class="diff">
              {#each diff as line}
                <div class="diff-line {line.kind}">
                  <span class="no mono">{line.oldNo ?? ""}</span>
                  <span class="no mono">{line.newNo ?? ""}</span>
                  <span class="sign mono">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>
                  <span class="text mono">{line.text || " "}</span>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </div>

    {#if publishOpen}
      <div class="panel publish">
        <div class="publish-head">
          <span class="mono-label">publish to github</span>
          <span class="badge warn">writes to the live profile</span>
        </div>
        <p class="publish-copy">
          Target: <span class="mono">{githubUser()?.login}/{githubUser()?.login}/README.md</span>.
          {#if remote?.exists === false}
            That repository has no README yet; publishing creates it.
          {:else}
            The live README is replaced with this draft in one commit.
          {/if}
          {#if remote?.sha} Based on the README currently live ({remote.sha.slice(0, 7)}).{/if}
        </p>
        <label class="commit-field">
          <span class="mono-label">Commit message</span>
          <input class="field" bind:value={commitMessage} maxlength="240" />
        </label>
        {#if publishError}<p class="error">{publishError}</p>{/if}
        <div class="publish-actions">
          <button class="btn ghost" onclick={() => (publishOpen = false)} disabled={publishing}>Cancel</button>
          <button class="btn primary" onclick={() => void publish()} disabled={publishing}>
            {publishing ? "Publishing…" : "Confirm publish"}
          </button>
        </div>
        <p class="hint">
          This is the only action that uploads profile content. The local draft remains available
          after publishing and is not deleted.
        </p>
      </div>
    {/if}

    {#if lastPublished}
      <p class="published mono">
        last publish → <button class="link" onclick={() => void api.openExternal(lastPublished ?? "").catch((cause) => notify(message(cause), "danger"))}>{lastPublished}</button>
      </p>
    {/if}
  {/if}
</section>

<style>
  .view-head {
    align-items: flex-start;
  }

  .view-head .view-sub {
    max-width: 80ch;
  }

  .head-status {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .studio {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--gap);
    align-items: start;
  }

  .editor,
  .preview {
    display: flex;
    flex-direction: column;
    min-height: 520px;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 9px;
    border-bottom: 1px solid var(--border);
  }

  .bar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 9px;
  }

  .bar .btn {
    height: 21px;
    padding: 0 7px;
    font-size: 9.5px;
  }

  .code-area {
    flex: 1;
    min-height: 430px;
    resize: vertical;
    border: 0;
    border-radius: 0;
    padding: 10px 11px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    line-height: 1.6;
    background: transparent;
    color: var(--text);
    tab-size: 2;
  }

  .code-area:focus {
    outline: none;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 9px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
  }

  .foot .btn {
    height: 22px;
    padding: 0 8px;
    font-size: 10px;
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
  }

  .seg.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .markdown {
    padding: 12px 14px;
    font-size: 11.5px;
    line-height: 1.65;
    color: var(--text-muted);
    overflow-wrap: break-word;
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

  .diff-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 5px 9px;
    border-bottom: 1px solid var(--border);
    font-size: 9.5px;
  }

  .diff {
    font-size: 9.5px;
    line-height: 1.6;
    padding: 4px 0;
    overflow-x: auto;
  }

  .diff-line {
    display: grid;
    grid-template-columns: 34px 34px 14px minmax(0, 1fr);
    gap: 4px;
    padding: 0 8px;
    white-space: pre;
  }

  .diff-line .no {
    color: var(--text-faint);
    text-align: right;
  }

  .diff-line.add {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .diff-line.del {
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .diff-line.add .sign,
  .add {
    color: var(--ok);
  }

  .diff-line.del .sign,
  .del {
    color: var(--danger);
  }

  .diff-line .text {
    color: var(--text-muted);
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
    padding: 12px 14px;
  }

  .notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    padding: 12px 14px;
    margin-bottom: var(--gap);
    font-size: var(--font-small);
  }

  .error {
    font-size: 10.5px;
    color: var(--danger);
    padding: 6px 10px;
  }

  .publish {
    margin-top: var(--gap);
    padding: 11px 13px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-left: 2px solid var(--warn);
  }

  .publish-head {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .publish-copy {
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-muted);
    max-width: 80ch;
  }

  .commit-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 460px;
  }

  .publish-actions {
    display: flex;
    gap: 6px;
  }

  .hint {
    font-size: 10px;
    color: var(--text-faint);
    line-height: 1.55;
  }

  .published {
    margin-top: 9px;
    font-size: 10px;
    color: var(--text-faint);
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

  @media (max-width: 1100px) {
    .studio {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
