<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { api, type ProfileReadme, type ProfileSnapshot } from "../lib/api";
  import { github, githubUser, isSignedIn, loadGithub } from "../lib/state/github.svelte";
  import { navigate, notify } from "../lib/state/ui.svelte";
  import { relativeTime } from "../lib/format";
  import { pushNotification } from "../lib/state/notifications.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import {
    ACHIEVEMENT_CATALOG,
    BADGE_STYLES,
    PRESETS,
    SECTION_LABELS,
    STAT_FIELDS,
    applyPreset,
    defaultDesign,
    normalizeDesign,
    type AchievementBadge,
    type ProfileDesign,
  } from "../lib/profile";

  const DRAFT_KEY = "profile.draft";
  const DRAFT_SAVED_KEY = "profile.draftSavedAt";
  const DESIGN_KEY = "profile.design";

  type Mode = "builder" | "draft";
  type DraftPane = "preview" | "diff";
  type BuilderPane = "preview" | "source";
  type DiffLine = { kind: "same" | "add" | "del"; text: string; oldNo: number | null; newNo: number | null };

  let design = $state<ProfileDesign>(defaultDesign());
  let designReady = $state(false);
  let designSavedAt = $state<number | null>(null);

  let draft = $state("");
  let draftWasMissing = false;
  let savedAt = $state<number | null>(null);
  let saving = $state(false);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let snapshot = $state<ProfileSnapshot | null>(null);
  let snapshotLoading = $state(false);
  let snapshotError = $state<string | null>(null);

  let generated = $state("");
  let generating = $state(false);
  let generationError = $state<string | null>(null);
  let autoSeedDone = false;

  let mode = $state<Mode>("builder");
  let builderPane = $state<BuilderPane>("preview");
  let draftPane = $state<DraftPane>("preview");

  let remote = $state<ProfileReadme | null>(null);
  let remoteLoading = $state(false);
  let remoteError = $state<string | null>(null);

  let publishOpen = $state(false);
  let commitMessage = $state("Update profile README");
  let publishing = $state(false);
  let publishError = $state<string | null>(null);
  let lastPublished = $state<string | null>(null);
  let createRepoOpen = $state(false);
  let creatingRepo = $state(false);
  let createRepoError = $state<string | null>(null);

  let statusEmoji = $state("");
  let statusMessage = $state("");
  let statusDirty = $state(false);
  let statusConfirm = $state(false);
  let statusSaving = $state(false);
  let statusError = $state<string | null>(null);
  let lastSnapshotFetch = 0;
  let statusDevice = $state<{
    code: string;
    deviceCode: string;
    expiresAt: number;
    interval: number;
  } | null>(null);
  let statusDeviceBusy = $state(false);
  let statusDeviceError = $state<string | null>(null);
  let statusPollTimer: ReturnType<typeof setTimeout> | null = null;
  let statusCancelled = true;

  let achievementName = $state("");
  let achievementTier = $state<AchievementBadge["tier"]>("bronze");

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let designTimer: ReturnType<typeof setTimeout> | null = null;
  let renderTimer: ReturnType<typeof setTimeout> | null = null;
  let renderTicket = 0;

  const readonly = $derived(!isSignedIn());
  const scopes = $derived(
    (github.status?.scopes ?? "")
      .split(",")
      .map((scope) => scope.trim())
      .filter(Boolean),
  );
  const canWriteStatus = $derived(scopes.length === 0 || scopes.includes("user"));

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
        const rawDesign = await api.getSetting(DESIGN_KEY);
        if (rawDesign) {
          try {
            design = normalizeDesign(JSON.parse(rawDesign));
          } catch {
            design = defaultDesign();
          }
        }
        const storedSavedAt = Number(await api.getSetting(DESIGN_KEY + "SavedAt"));
        designSavedAt = Number.isFinite(storedSavedAt) && storedSavedAt > 0 ? storedSavedAt : null;

        const rawDraft = await api.getSetting(DRAFT_KEY);
        draftWasMissing = rawDraft === null;
        draft = rawDraft ?? "";
        const stored = Number(await api.getSetting(DRAFT_SAVED_KEY));
        savedAt = Number.isFinite(stored) && stored > 0 ? stored : null;
      } catch (cause) {
        loadError = message(cause);
      } finally {
        loading = false;
        designReady = true;
      }
      void loadRemote();
      void loadSnapshot();
    })();
  });

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
    if (designTimer) clearTimeout(designTimer);
    if (renderTimer) clearTimeout(renderTimer);
    if (statusPollTimer) clearTimeout(statusPollTimer);
    statusCancelled = true;
  });

  function refreshOnFocus(): void {
    if (!isSignedIn() || statusDirty || snapshotLoading) return;
    if (Date.now() - lastSnapshotFetch < 60_000) return;
    void loadSnapshot();
  }

  onMount(() => {
    window.addEventListener("focus", refreshOnFocus);
    return () => window.removeEventListener("focus", refreshOnFocus);
  });

  $effect(() => {
    const encoded = JSON.stringify(design);
    if (!designReady) return;
    if (designTimer) clearTimeout(designTimer);
    designTimer = setTimeout(() => {
      void api
        .setSetting(DESIGN_KEY, encoded)
        .then(async () => {
          designSavedAt = Date.now();
          await api.setSetting(DESIGN_KEY + "SavedAt", String(designSavedAt));
        })
        .catch((cause) => notify(message(cause), "danger"));
    }, 450);
  });

  $effect(() => {
    const currentDesign = JSON.stringify(design);
    const currentSnapshot = snapshot;
    if (!currentSnapshot) return;
    void currentDesign;
    if (renderTimer) clearTimeout(renderTimer);
    renderTimer = setTimeout(() => void renderGenerated(), 100);
  });

  async function renderGenerated(): Promise<void> {
    if (!snapshot) {
      generated = "";
      return;
    }
    const ticket = ++renderTicket;
    generating = true;
    generationError = null;
    try {
      const markdown = await api.profileRender(
        $state.snapshot(design) as ProfileDesign,
        snapshot,
      );
      if (ticket !== renderTicket) return;
      generated = markdown;
      if (draftWasMissing && !autoSeedDone && !draft.trim() && markdown.trim()) {
        autoSeedDone = true;
        draft = markdown;
        await saveDraft(true);
        notify("Generated a profile README draft from your GitHub data. Nothing was published.", "ok");
      }
    } catch (cause) {
      if (ticket !== renderTicket) return;
      generationError = message(cause);
    } finally {
      if (ticket === renderTicket) generating = false;
    }
  }

  async function loadSnapshot(): Promise<void> {
    if (!isSignedIn()) {
      snapshot = null;
      return;
    }
    snapshotLoading = true;
    snapshotError = null;
    try {
      const next = await api.githubProfileSnapshot();
      snapshot = next;
      lastSnapshotFetch = Date.now();
      if (!statusDirty) {
        statusEmoji = next.status?.emoji ?? "";
        statusMessage = next.status?.message ?? "";
      }
    } catch (cause) {
      snapshotError = message(cause);
    } finally {
      snapshotLoading = false;
    }
  }

  function markStatusDirty(): void {
    statusDirty = true;
    statusConfirm = false;
    statusError = null;
  }

  function useStatusEmoji(emoji: string): void {
    statusEmoji = emoji;
    markStatusDirty();
  }

  function prepareClearStatus(): void {
    statusEmoji = "";
    statusMessage = "";
    statusDirty = true;
    statusError = null;
    statusConfirm = true;
  }

  async function startStatusDevice(): Promise<void> {
    statusDeviceError = null;
    statusDeviceBusy = true;
    try {
      const start = await api.startDeviceFlow();
      statusDevice = {
        code: start.userCode,
        deviceCode: start.deviceCode,
        expiresAt: Date.now() + start.expiresIn * 1000,
        interval: Math.max(5, start.interval),
      };
      statusCancelled = false;
      void api
        .openExternal(start.verificationUri)
        .catch((cause) => (statusDeviceError = message(cause)));
      scheduleStatusPoll(statusDevice.interval);
    } catch (cause) {
      statusDeviceError = message(cause);
    } finally {
      statusDeviceBusy = false;
    }
  }

  function scheduleStatusPoll(seconds: number): void {
    if (statusPollTimer) clearTimeout(statusPollTimer);
    statusPollTimer = setTimeout(
      () => void pollStatusDevice(),
      Math.max(5, seconds) * 1000,
    );
  }

  function cancelStatusDevice(): void {
    statusCancelled = true;
    if (statusPollTimer) clearTimeout(statusPollTimer);
    statusDevice = null;
    statusDeviceError = null;
  }

  async function pollStatusDevice(): Promise<void> {
    const current = statusDevice;
    if (!current || statusCancelled) return;
    if (Date.now() > current.expiresAt) {
      statusDeviceError = "The device code expired. Start again.";
      statusDevice = null;
      return;
    }
    try {
      const result = await api.pollDeviceFlow(current.deviceCode);
      if (result.status === "pending") {
        scheduleStatusPoll(current.interval);
      } else if (result.status === "slowDown") {
        current.interval = result.interval + 5;
        scheduleStatusPoll(current.interval);
      } else if (result.status === "failed") {
        statusDeviceError = result.message;
        statusDevice = null;
      } else {
        statusCancelled = true;
        statusDevice = null;
        if (statusPollTimer) clearTimeout(statusPollTimer);
        await loadGithub();
        await loadSnapshot();
        notify("Status access granted to GRAFT", "ok");
      }
    } catch (cause) {
      statusDeviceError = message(cause);
      statusDevice = null;
    }
  }

  async function applyStatus(): Promise<void> {
    if (statusSaving || !isSignedIn()) return;
    statusSaving = true;
    statusError = null;
    try {
      const next = await api.setProfileStatus(
        statusEmoji.trim() || null,
        statusMessage.trim(),
      );
      if (snapshot) snapshot = { ...snapshot, status: next };
      statusDirty = false;
      statusConfirm = false;
      statusEmoji = next?.emoji ?? "";
      statusMessage = next?.message ?? "";
      pushNotification({
        title: next ? "GitHub status updated" : "GitHub status cleared",
        detail: next ? `${next.emoji} ${next.message}`.trim() : "Your profile no longer has a status",
        tone: "ok",
        target: { view: "profile" },
      });
    } catch (cause) {
      statusError = message(cause);
    } finally {
      statusSaving = false;
    }
  }

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

  function useGenerated(): void {
    if (!generated.trim()) return;
    draft = generated;
    draftWasMissing = false;
    void saveDraft(true);
    mode = "draft";
    draftPane = "preview";
    notify("Generated README loaded into the local draft", "ok");
  }

  async function copyGenerated(): Promise<void> {
    if (!generated) return;
    try {
      await navigator.clipboard.writeText(generated);
      notify("Markdown copied", "ok");
    } catch {
      notify("The clipboard is not available in this webview.", "danger");
    }
  }

  function choosePreset(id: string): void {
    design = applyPreset($state.snapshot(design) as ProfileDesign, id);
  }

  function moveSection(index: number, delta: number): void {
    const target = index + delta;
    if (target < 0 || target >= design.sections.length) return;
    const next = [...design.sections];
    const [item] = next.splice(index, 1);
    next.splice(target, 0, item);
    design.sections = next;
  }

  function toggleStatField(id: string): void {
    const fields = design.stats.fields;
    design.stats.fields = fields.includes(id)
      ? fields.filter((field) => field !== id)
      : [...fields, id];
  }

  function addAchievement(): void {
    const name = achievementName.trim();
    if (!name) return;
    if (design.achievements.badges.some((badge) => badge.name.toLowerCase() === name.toLowerCase())) {
      notify("That badge is already on the profile.", "danger");
      return;
    }
    design.achievements.badges = [
      ...design.achievements.badges,
      { name: name.slice(0, 60), tier: achievementTier },
    ];
    achievementName = "";
  }

  function removeAchievement(index: number): void {
    design.achievements.badges = design.achievements.badges.filter((_, item) => item !== index);
  }

  function useRemote(): void {
    if (remote?.content == null) return;
    draft = remote.content;
    draftWasMissing = false;
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
    draftWasMissing = false;
    void saveDraft(true);
  }

  function openProfile(): void {
    const url = githubUser()?.htmlUrl;
    if (!url) return;
    void api.openExternal(url).catch((cause) => notify(message(cause), "danger"));
  }

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

  async function createProfileRepo(): Promise<void> {
    const user = githubUser();
    if (!user || creatingRepo) return;
    creatingRepo = true;
    createRepoError = null;
    try {
      const result = await api.createProfileRepo(user.login);
      createRepoOpen = false;
      remote = {
        exists: false,
        content: null,
        sha: null,
        htmlUrl: result.htmlUrl,
      };
      await loadRemote();
      pushNotification({
        title: "Profile repository created",
        detail: result.fullName,
        tone: "ok",
        target: { view: "profile" },
      });
    } catch (cause) {
      createRepoError = message(cause);
    } finally {
      creatingRepo = false;
    }
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
      pushNotification({
        title: "Profile README published",
        detail: `${user.login}/${user.login}`,
        tone: "ok",
        target: { view: "profile" },
      });
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
        Design the profile from your real GitHub data, then keep it as a local draft.
        Nothing reaches GitHub until you confirm a publish.
      </p>
    </div>
    <div class="head-status">
      {#if isSignedIn() && githubUser()}
        <span class="badge ok">@{githubUser()?.login}</span>
        <button class="btn ghost" onclick={() => void loadSnapshot()} disabled={snapshotLoading}>
          {snapshotLoading ? "Reading…" : "Refresh data"}
        </button>
        <button class="btn ghost" onclick={openProfile}>Open profile ↗</button>
      {:else}
        <span class="badge warn">draft only</span>
      {/if}
    </div>
  </div>

  {#if loading}
    <p class="state mono">reading local design…</p>
  {:else if loadError}
    <div class="panel notice">
      <p class="tone-danger">Profile Studio could not be loaded.</p>
      <p class="faint">{loadError}</p>
    </div>
  {:else}
    {#if readonly}
      <div class="panel notice">
        <p>The builder can be configured without an account, but live profile data needs GitHub.</p>
        <button class="btn" onclick={() => navigate("repositories")}>Connect GitHub</button>
      </div>
    {/if}

    {#if isSignedIn() && remote?.exists === false}
      <div class="panel repo-notice">
        <div>
          <span class="mono-label">profile repository missing</span>
          <p class="faint">
            GitHub publishes a profile README from <span class="mono">{githubUser()?.login}/{githubUser()?.login}</span>.
            Create that public repository now, or continue designing locally.
          </p>
        </div>
        <button class="btn ghost" onclick={() => (createRepoOpen = !createRepoOpen)}>
          {createRepoOpen ? "Close" : "Create profile repository"}
        </button>
        {#if createRepoOpen}
          <div class="repo-confirm">
            <span class="badge warn">external write · public repository</span>
            <p class="faint">
              GRAFT will ask GitHub to create <span class="mono">{githubUser()?.login}/{githubUser()?.login}</span> with an initial README.
              Nothing is published to the profile automatically.
            </p>
            {#if createRepoError}<p class="error">{createRepoError}</p>{/if}
            <div class="row">
              <button class="btn ghost" onclick={() => (createRepoOpen = false)} disabled={creatingRepo}>Cancel</button>
              <button class="btn primary" onclick={() => void createProfileRepo()} disabled={creatingRepo}>
                {creatingRepo ? "Creating…" : "Confirm create"}
              </button>
            </div>
          </div>
        {/if}
      </div>
    {/if}

    <div class="modebar">
      <div class="segmented">
        <button class="seg" class:active={mode === "builder"} onclick={() => (mode = "builder")}>Builder</button>
        <button class="seg" class:active={mode === "draft"} onclick={() => (mode = "draft")}>Draft</button>
      </div>
      <div class="mode-actions">
        <span class="mono faint">
          {#if designSavedAt}design saved {relativeTime(Math.floor(designSavedAt / 1000))}{:else}design local{/if}
          {#if savedAt} · draft saved {relativeTime(Math.floor(savedAt / 1000))}{/if}
        </span>
        <button class="btn ghost" onclick={() => void copyGenerated()} disabled={!generated}>Copy Markdown</button>
        <button class="btn primary" onclick={useGenerated} disabled={!generated.trim()}>Use as draft</button>
      </div>
    </div>

    {#if mode === "builder"}
      <div class="preset-strip">
        {#each PRESETS as preset}
          <button
            class="preset"
            class:active={design.preset === preset.id}
            onclick={() => choosePreset(preset.id)}
          >
            <strong>{preset.label}</strong>
            <span>{preset.description}</span>
          </button>
        {/each}
      </div>

      {#if !isSignedIn()}
        <div class="panel builder-empty">
          <p>Connect GitHub to fill the builder with your avatar, pins, stats and activity calendar.</p>
          <p class="faint">The design itself is saved locally and can be prepared first.</p>
        </div>
      {:else if snapshotLoading}
        <p class="state mono">reading profile, pins and contribution calendar…</p>
      {:else if snapshotError}
        <div class="panel notice">
          <p class="tone-danger">Live profile data could not be read.</p>
          <p class="faint">{snapshotError}</p>
          <button class="btn ghost" onclick={() => void loadSnapshot()}>Retry</button>
        </div>
      {:else if !snapshot}
        <p class="state">No profile snapshot yet.</p>
      {/if}

      <div class="builder">
        <div class="panel controls">
          <div class="controls-head">
            <span class="mono-label">design</span>
            <button class="btn ghost" onclick={() => (design = applyPreset($state.snapshot(design) as ProfileDesign, "everything"))}>
              Reset layout
            </button>
          </div>

          <div class="control-group">
            <div class="group-head">
              <span class="mono-label">github status</span>
              <span class="mono faint">
                {#if snapshot?.status}
                  {snapshot.status.emoji} {snapshot.status.message}
                {:else}
                  not set
                {/if}
              </span>
            </div>
            <div class="status-inputs">
              <input
                class="field status-emoji"
                bind:value={statusEmoji}
                oninput={markStatusDirty}
                maxlength="16"
                placeholder="🚀"
                aria-label="Status emoji"
              />
              <input
                class="field"
                bind:value={statusMessage}
                oninput={markStatusDirty}
                maxlength="80"
                placeholder="What are you working on?"
                aria-label="Status message"
              />
            </div>
            <div class="chips">
              {#each ["🚀", "🌱", "🛠️", "🎧", "🌙", "☕"] as emoji}
                <button class="chip emoji-chip" onclick={() => useStatusEmoji(emoji)} title={`Use ${emoji}`}>{emoji}</button>
              {/each}
            </div>
            {#if !canWriteStatus}
              <div class="status-scope">
                <span class="badge warn">user scope missing</span>
                <p>
                  GitHub only accepts status writes from a token with the <span class="mono">user</span>
                  scope. Your current scopes are <span class="mono">{scopes.join(", ") || "unknown"}</span>.
                </p>
                {#if statusDevice}
                  <div class="status-device">
                    <span class="mono-label">enter this code on github.com</span>
                    <strong class="mono">{statusDevice.code}</strong>
                    <span class="waiting mono-label">waiting for approval…</span>
                  </div>
                  <div class="row">
                    <button class="btn ghost" onclick={cancelStatusDevice}>Cancel</button>
                    <button class="btn" onclick={() => void api.openExternal("https://github.com/login/device").catch((cause) => (statusDeviceError = message(cause)))}>Open device page ↗</button>
                  </div>
                {:else}
                  <button class="btn ghost" onclick={() => void startStatusDevice()} disabled={statusDeviceBusy}>
                    {statusDeviceBusy ? "Requesting code…" : "Grant status access"}
                  </button>
                {/if}
                {#if statusDeviceError}<p class="error">{statusDeviceError}</p>{/if}
              </div>
            {/if}
            {#if statusConfirm}
              <div class="status-confirm">
                <span class="badge warn">live GitHub write</span>
                <p>
                  {statusEmoji || statusMessage
                    ? `Set your status to ${statusEmoji} ${statusMessage}`.trim()
                    : "Remove the status from your GitHub profile."}
                </p>
                {#if statusError}<p class="error">{statusError}</p>{/if}
                <div class="row">
                  <button class="btn ghost" onclick={() => (statusConfirm = false)} disabled={statusSaving}>Cancel</button>
                  <button class="btn primary" onclick={() => void applyStatus()} disabled={statusSaving || !canWriteStatus}>
                    {statusSaving ? "Updating…" : "Confirm status"}
                  </button>
                </div>
              </div>
            {:else}
              <div class="row">
                <button
                  class="btn ghost"
                  onclick={() => (statusConfirm = true)}
                  disabled={statusSaving || !canWriteStatus || (!statusEmoji.trim() && !statusMessage.trim() && !snapshot?.status)}
                >
                  {snapshot?.status ? "Update status" : "Set status"}
                </button>
                {#if snapshot?.status || statusEmoji || statusMessage}
                  <button class="btn ghost" onclick={prepareClearStatus} disabled={statusSaving}>Clear</button>
                {/if}
              </div>
            {/if}
            {#if statusError && !statusConfirm}<p class="error">{statusError}</p>{/if}
            <p class="hint">
              Writes the status to your live GitHub profile, and to this README when Status is enabled.
              GitHub requires the <span class="mono">user</span> scope; re-authorise if the token predates it.
            </p>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">identity</span></div>
            <label class="check">
              <input type="checkbox" bind:checked={design.header.avatar} />
              <span>Show profile picture</span>
            </label>
            <div class="range-row">
              <span class="mono faint">avatar</span>
              <input type="range" min="48" max="240" step="8" bind:value={design.header.avatarSize} />
              <span class="mono">{design.header.avatarSize}px</span>
            </div>
            <input
              class="field"
              bind:value={design.header.tagline}
              maxlength="140"
              placeholder="Tagline — one line under your name"
              aria-label="Profile tagline"
            />
            <div class="chips">
              <label class="chip"><input type="checkbox" bind:checked={design.header.showName} /> Name</label>
              <label class="chip"><input type="checkbox" bind:checked={design.header.showLogin} /> @login</label>
              <label class="chip"><input type="checkbox" bind:checked={design.header.showStatus} /> Status</label>
            </div>
            <label class="check">
              <input type="checkbox" bind:checked={design.about.useGithubBio} />
              <span>Use GitHub bio</span>
            </label>
            <textarea
              class="field area"
              bind:value={design.about.bioOverride}
              disabled={design.about.useGithubBio}
              maxlength="600"
              placeholder={snapshot?.bio ?? "Bio override"}
              aria-label="Bio override"
            ></textarea>
            <div class="chips">
              <label class="chip"><input type="checkbox" bind:checked={design.about.company} /> Company</label>
              <label class="chip"><input type="checkbox" bind:checked={design.about.location} /> Location</label>
              <label class="chip"><input type="checkbox" bind:checked={design.about.website} /> Website</label>
              <label class="chip"><input type="checkbox" bind:checked={design.about.joined} /> Joined</label>
              <label class="chip"><input type="checkbox" bind:checked={design.about.socials} /> Socials</label>
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">layout + theme</span></div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">layout</span>
                <select class="field" bind:value={design.layout}>
                  <option value="banner">Banner</option>
                  <option value="split">Split</option>
                  <option value="compact">Compact</option>
                  <option value="terminal">Terminal</option>
                </select>
              </label>
              <label class="field-label">
                <span class="mono faint">align</span>
                <select class="field" bind:value={design.align}>
                  <option value="center">Center</option>
                  <option value="left">Left</option>
                </select>
              </label>
            </div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">accent</span>
                <span class="accent-row">
                  <input
                    type="color"
                    value={`#${design.accent}`}
                    oninput={(event) => {
                      design.accent = (event.currentTarget as HTMLInputElement).value.replace("#", "");
                    }}
                  />
                  <input class="field mono" bind:value={design.accent} maxlength="7" aria-label="Accent hex" />
                </span>
              </label>
              <label class="check inline">
                <input type="checkbox" bind:checked={design.showHeadings} />
                <span>Section headings</span>
              </label>
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">sections · order</span></div>
            <div class="section-list">
              {#each design.sections as section, index (section.id)}
                <div class="section-row">
                  <input type="checkbox" bind:checked={section.enabled} aria-label={`Show ${SECTION_LABELS[section.id]}`} />
                  <span class="section-name">{SECTION_LABELS[section.id] ?? section.id}</span>
                  <button class="btn ghost icon" title="Move up" aria-label="Move up" onclick={() => moveSection(index, -1)} disabled={index === 0}>
                    <Icon name="chevron-up" size={11} />
                  </button>
                  <button class="btn ghost icon" title="Move down" aria-label="Move down" onclick={() => moveSection(index, 1)} disabled={index === design.sections.length - 1}>
                    <Icon name="chevron-down" size={11} />
                  </button>
                </div>
              {/each}
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">stats</span></div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">style</span>
                <select class="field" bind:value={design.stats.style}>
                  <option value="badges">Badges</option>
                  <option value="table">Table</option>
                  <option value="inline">Inline</option>
                </select>
              </label>
              <label class="field-label">
                <span class="mono faint">badge</span>
                <select class="field" bind:value={design.stats.badgeStyle}>
                  {#each BADGE_STYLES as badge}
                    <option value={badge.id}>{badge.label}</option>
                  {/each}
                </select>
              </label>
            </div>
            <div class="chips">
              {#each STAT_FIELDS as field}
                <label class="chip">
                  <input
                    type="checkbox"
                    checked={design.stats.fields.includes(field.id)}
                    onchange={() => toggleStatField(field.id)}
                  />
                  {field.label}
                </label>
              {/each}
            </div>
            {#if snapshot && snapshot.stars === 0}
              <p class="hint">Stars are real: currently zero across the repositories in this snapshot.</p>
            {/if}
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">pinned repositories</span></div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">source</span>
                <select class="field" bind:value={design.pinned.source}>
                  <option value="pinned">GitHub pins</option>
                  <option value="top">Top starred</option>
                </select>
              </label>
              <label class="field-label">
                <span class="mono faint">style</span>
                <select class="field" bind:value={design.pinned.style}>
                  <option value="cards">Cards</option>
                  <option value="list">List</option>
                  <option value="table">Table</option>
                </select>
              </label>
            </div>
            <div class="range-row">
              <span class="mono faint">count</span>
              <input type="range" min="1" max="6" step="1" bind:value={design.pinned.limit} />
              <span class="mono">{design.pinned.limit}</span>
            </div>
            <div class="chips">
              <label class="chip"><input type="checkbox" bind:checked={design.pinned.description} /> Description</label>
              <label class="chip"><input type="checkbox" bind:checked={design.pinned.language} /> Language</label>
              <label class="chip"><input type="checkbox" bind:checked={design.pinned.stars} /> Stars</label>
              <label class="chip"><input type="checkbox" bind:checked={design.pinned.forks} /> Forks</label>
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">activity map</span></div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">style</span>
                <select class="field" bind:value={design.contributions.style}>
                  <option value="heatmap">Emoji heatmap</option>
                  <option value="ascii">ASCII heatmap</option>
                  <option value="badge">Badge</option>
                  <option value="summary">Summary only</option>
                </select>
              </label>
              <div class="range-row compact">
                <span class="mono faint">weeks</span>
                <input type="range" min="8" max="53" step="1" bind:value={design.contributions.weeks} />
                <span class="mono">{design.contributions.weeks}</span>
              </div>
            </div>
            <div class="chips">
              <label class="chip"><input type="checkbox" bind:checked={design.contributions.showTotals} /> Totals</label>
              <label class="chip"><input type="checkbox" bind:checked={design.contributions.showBreakdown} /> Breakdown</label>
              <label class="chip"><input type="checkbox" bind:checked={design.contributions.showStreak} /> Streak</label>
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">achievements</span></div>
            <p class="hint">
              GitHub exposes no achievements API. Add the badges you have actually earned; they
              link to your achievements tab.
            </p>
            <div class="achievement-add">
              <input
                class="field"
                bind:value={achievementName}
                list="achievement-catalog"
                maxlength="60"
                placeholder="Badge name"
                aria-label="Achievement badge name"
              />
              <datalist id="achievement-catalog">
                {#each ACHIEVEMENT_CATALOG as name}
                  <option value={name}></option>
                {/each}
              </datalist>
              <select class="field" bind:value={achievementTier} aria-label="Achievement tier">
                <option value="bronze">Bronze</option>
                <option value="silver">Silver</option>
                <option value="gold">Gold</option>
              </select>
              <button class="btn ghost" onclick={addAchievement} disabled={!achievementName.trim()}>Add</button>
            </div>
            {#if design.achievements.badges.length > 0}
              <div class="badge-list">
                {#each design.achievements.badges as badge, index (badge.name + badge.tier)}
                  <span class="badge-item">
                    {badge.name} · {badge.tier}
                    <button class="link" aria-label={`Remove ${badge.name}`} onclick={() => removeAchievement(index)}>×</button>
                  </span>
                {/each}
              </div>
            {/if}
            <label class="check">
              <input type="checkbox" bind:checked={design.achievements.showLink} />
              <span>Link to the achievements tab</span>
            </label>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">organizations</span></div>
            <div class="two">
              <label class="field-label">
                <span class="mono faint">style</span>
                <select class="field" bind:value={design.organizations.style}>
                  <option value="avatars">Avatars</option>
                  <option value="list">List</option>
                </select>
              </label>
              <div class="range-row compact">
                <span class="mono faint">limit</span>
                <input type="range" min="1" max="12" step="1" bind:value={design.organizations.limit} />
                <span class="mono">{design.organizations.limit}</span>
              </div>
            </div>
          </div>

          <div class="control-group">
            <div class="group-head"><span class="mono-label">custom markdown + footer</span></div>
            <textarea
              class="field area tall"
              bind:value={design.customMarkdown}
              placeholder="## Elsewhere&#10;&#10;Anything GitHub-flavoured Markdown."
              aria-label="Custom Markdown"
            ></textarea>
            <input class="field" bind:value={design.footer} maxlength="200" placeholder="Footer line" aria-label="Footer" />
          </div>
        </div>

        <div class="panel output">
          <div class="bar">
            <div class="segmented">
              <button class="seg" class:active={builderPane === "preview"} onclick={() => (builderPane = "preview")}>Preview</button>
              <button class="seg" class:active={builderPane === "source"} onclick={() => (builderPane = "source")}>Markdown</button>
            </div>
            <span class="mono faint">
              {#if generating}
                rendering…
              {:else}
                {generated.split("\n").length} lines · local only
              {/if}
            </span>
          </div>

          {#if generationError}
            <p class="error">{generationError}</p>
          {/if}

          {#if builderPane === "preview"}
            {#if generated.trim()}
              <div class="markdown">{@html render(generated)}</div>
            {:else}
              <p class="state">The preview appears when live profile data is available.</p>
            {/if}
          {:else}
            <pre class="source mono">{generated || "—"}</pre>
          {/if}

          <div class="output-foot">
            <span class="mono faint">generated from live data · editable as Markdown</span>
            <div class="row">
              <button class="btn ghost" onclick={() => void copyGenerated()} disabled={!generated}>Copy</button>
              <button class="btn" onclick={useGenerated} disabled={!generated.trim()}>Use as draft</button>
              <button class="btn ghost" onclick={() => (mode = "draft")}>Draft & diff →</button>
            </div>
          </div>
        </div>
      </div>
    {:else}
      <div class="studio">
        <div class="panel editor">
          <div class="bar">
            <span class="mono-label">draft · markdown</span>
            <div class="bar-right">
              <span class="mono faint">{draft.split("\n").length} lines · {draft.length} chars</span>
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
              {#if generated.trim()}
                <button class="btn ghost" onclick={useGenerated}>Generated README</button>
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
              <button class="seg" class:active={draftPane === "preview"} onclick={() => (draftPane = "preview")}>Preview</button>
              <button class="seg" class:active={draftPane === "diff"} onclick={() => (draftPane = "diff")}>Diff</button>
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

          {#if draftPane === "preview"}
            {#if draft.trim()}
              <div class="markdown">{@html render(draft)}</div>
            {:else}
              <p class="state">Nothing to preview yet. Generate from the Builder, write Markdown, or start from the template.</p>
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
  {/if}
</section>

<style>
  .view-head {
    align-items: flex-start;
  }

  .view-head .view-sub {
    max-width: 82ch;
  }

  .head-status {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .repo-notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    padding: 9px 11px;
    margin-bottom: var(--gap);
    font-size: 10.5px;
  }

  .repo-notice > div:first-child {
    min-width: 220px;
    flex: 1;
  }

  .repo-notice p {
    margin-top: 2px;
    line-height: 1.5;
  }

  .repo-notice .btn {
    height: 24px;
    padding: 0 8px;
    font-size: 10px;
  }

  .repo-confirm {
    flex-basis: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 9px;
    border-left: 2px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 4%, transparent);
  }

  .modebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: var(--gap);
    flex-wrap: wrap;
  }

  .mode-actions {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    font-size: 10px;
  }

  .mode-actions .btn {
    height: 23px;
    padding: 0 8px;
    font-size: 10px;
  }

  .preset-strip {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
    margin-bottom: var(--gap);
  }

  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 7px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .preset strong {
    color: var(--text);
    font-size: 10.5px;
    font-weight: 600;
  }

  .preset span {
    color: var(--text-faint);
    font-size: 9.5px;
    line-height: 1.4;
  }

  .preset:hover,
  .preset.active {
    border-color: var(--border-strong);
    background: var(--bg-elev);
  }

  .preset.active {
    border-left: 2px solid var(--accent);
  }

  .builder-empty {
    padding: 11px 13px;
    margin-bottom: var(--gap);
    font-size: var(--font-small);
  }

  .builder {
    display: grid;
    grid-template-columns: 385px minmax(0, 1fr);
    gap: var(--gap);
    align-items: start;
  }

  .controls {
    max-height: calc(100vh - 260px);
    min-height: 480px;
    overflow-y: auto;
  }

  .controls-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 9px;
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--surface);
  }

  .controls-head .btn {
    height: 21px;
    padding: 0 7px;
    font-size: 9.5px;
  }

  .control-group {
    padding: 9px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .control-group:last-child {
    border-bottom: 0;
  }

  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: var(--text-muted);
    cursor: pointer;
  }

  .check.inline {
    align-self: end;
    padding-bottom: 4px;
  }

  .status-inputs {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr);
    gap: 5px;
  }

  .status-inputs .field {
    height: 25px;
    font-size: 10.5px;
  }

  .status-emoji {
    text-align: center;
    font-size: 14px;
  }

  .emoji-chip {
    border: 1px solid var(--border);
    background: transparent;
    font: inherit;
    font-size: 13px;
    line-height: 1;
    padding: 2px 5px;
    cursor: pointer;
  }

  .emoji-chip:hover {
    border-color: var(--border-strong);
  }

  .status-confirm,
  .status-scope {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 6px 7px;
    border-left: 2px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 4%, transparent);
  }

  .status-confirm p,
  .status-scope p {
    font-size: 10px;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .status-device {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    padding: 5px 6px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
  }

  .status-device strong {
    font-size: 14px;
    letter-spacing: 0.12em;
    color: var(--accent);
  }

  .waiting {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9px;
  }

  .range-row {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 9.5px;
  }

  .range-row input[type="range"] {
    flex: 1;
    accent-color: var(--accent);
  }

  .range-row.compact {
    flex-wrap: wrap;
  }

  .two {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 7px;
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 9.5px;
  }

  .field-label .field {
    height: 24px;
    font-size: 10.5px;
  }

  .accent-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .accent-row input[type="color"] {
    width: 26px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    cursor: pointer;
  }

  .accent-row .field {
    flex: 1;
    min-width: 0;
  }

  .area {
    min-height: 62px;
    resize: vertical;
    font-size: 10.5px;
    line-height: 1.55;
    padding: 6px 8px;
  }

  .area.tall {
    min-height: 96px;
    font-family: var(--font-mono);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 9.5px;
    cursor: pointer;
  }

  .chip:hover {
    border-color: var(--border-strong);
  }

  .section-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .section-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 0 2px;
  }

  .section-name {
    flex: 1;
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .section-row .btn.icon {
    width: 20px;
    height: 20px;
    padding: 0;
  }

  .achievement-add {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 74px auto;
    gap: 5px;
  }

  .achievement-add .field {
    height: 24px;
    font-size: 10.5px;
  }

  .achievement-add .btn {
    height: 24px;
    padding: 0 8px;
    font-size: 9.5px;
  }

  .badge-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .badge-item {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 9.5px;
    color: var(--text-muted);
  }

  .badge-item .link {
    font-size: 12px;
    line-height: 1;
  }

  .hint {
    font-size: 9.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }

  .output {
    min-height: 520px;
    display: flex;
    flex-direction: column;
    position: sticky;
    top: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 9px;
    border-bottom: 1px solid var(--border);
  }

  .output .markdown {
    flex: 1;
  }

  .source {
    flex: 1;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    font-size: 10px;
    line-height: 1.65;
    color: var(--text-muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .output-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 9px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
    font-size: 9.5px;
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
    min-height: 540px;
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
    min-height: 450px;
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

  @media (max-width: 1250px) {
    .builder {
      grid-template-columns: 340px minmax(0, 1fr);
    }

    .preset-strip {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 1050px) {
    .builder {
      grid-template-columns: minmax(0, 1fr);
    }

    .controls {
      max-height: none;
    }

    .output {
      position: static;
    }

    .studio {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
