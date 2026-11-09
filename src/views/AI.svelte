<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { onMount } from "svelte";
  import { api, type Project } from "../lib/api";
  import {
    ai,
    attachFile,
    canSend,
    cancelMessage,
    clearContext,
    clearKey,
    deleteConversation,
    isAttached,
    keyConfigured,
    keyError,
    loadAI,
    loadContextFiles,
    needsConsent,
    newConversation,
    openConversation,
    removeContext,
    saveKey,
    selectProvider,
    sendMessage,
    setConsent,
    setContextProject,
    setEndpoint,
    setModel,
    setSystem,
    usesLocalEndpoint,
  } from "../lib/state/ai.svelte";
  import { notify } from "../lib/state/ui.svelte";

  let projects = $state<Project[]>([]);
  let projectsLoading = $state(false);
  let keyInput = $state("");
  let showKey = $state(false);
  let fileFilter = $state("");
  let messagesEl: HTMLDivElement | undefined = $state();
  let autoScroll = $state(true);

  const current = $derived(ai.providers.find((item) => item.id === ai.providerId));
  const status = $derived(ai.keys[ai.providerId]);
  const filteredFiles = $derived(
    ai.contextFiles.filter((file) =>
      file.path.toLowerCase().includes(fileFilter.trim().toLowerCase()),
    ),
  );
  const sendable = $derived(canSend());
  const consentNeeded = $derived(needsConsent());
  const localEndpoint = $derived(usesLocalEndpoint());

  onMount(() => {
    void (async () => {
      projectsLoading = true;
      try {
        projects = await api.listProjects();
      } catch (cause) {
        notify(cause instanceof Error ? cause.message : String(cause), "danger");
      } finally {
        projectsLoading = false;
      }
      await loadAI();
      if (!ai.context.length && !ai.draft) {
        ai.error = null;
      }
    })();
  });

  $effect(() => {
    if (!messagesEl || !autoScroll) return;
    ai.messages.length;
    ai.streamText;
    messagesEl.scrollTop = messagesEl.scrollHeight;
  });

  function render(source: string): string {
    const parsed = marked.parse(source, { async: false, gfm: true });
    return DOMPurify.sanitize(typeof parsed === "string" ? parsed : "");
  }

  function chooseProject(value: string): void {
    const id = value ? Number(value) : null;
    setContextProject(Number.isFinite(id) ? id : null);
    void loadContextFiles();
  }

  async function saveProviderKey(): Promise<void> {
    if (await saveKey(keyInput)) keyInput = "";
  }

  async function removeProviderKey(): Promise<void> {
    if (!confirm(`Remove the stored key for ${current?.label ?? ai.providerId}?`)) return;
    await clearKey();
  }

  async function send(): Promise<void> {
    if (!sendable) return;
    autoScroll = true;
    await sendMessage();
  }

  async function removeConversation(id: number, title: string): Promise<void> {
    if (!confirm(`Delete the conversation “${title}”?`)) return;
    await deleteConversation(id);
  }

  function onComposerKeydown(event: KeyboardEvent): void {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
    event.preventDefault();
    void send();
  }

  function onMessagesScroll(): void {
    if (!messagesEl) return;
    const distance =
      messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight;
    autoScroll = distance < 60;
  }

  function relative(epoch: number): string {
    const seconds = Math.max(0, Math.floor(Date.now() / 1000) - epoch);
    if (seconds < 60) return "just now";
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
  }

  function size(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} kB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<section class="view ai-view">
  <div class="view-head">
    <div>
      <h1 class="view-title">AI Workspace</h1>
      <p class="view-sub">
        Your providers, your keys, your context. Chat runs from the Rust engine and streams
        back token by token; GRAFT stays fully usable without it.
      </p>
    </div>
    <div class="head-actions">
      <span class="badge" class:ok={keyConfigured()} class:warn={!keyConfigured() && current?.keyRequired}>
        {keyConfigured() ? "key stored" : current?.keyRequired ? "key missing" : "no key needed"}
      </span>
      <button class="btn" onclick={newConversation} disabled={ai.streaming}>New chat</button>
    </div>
  </div>

  <div class="ai-grid">
    <aside class="side">
      <section class="panel provider-panel">
        <div class="panel-head">
          <span class="mono-label">provider</span>
          <span class="badge" class:ok={localEndpoint}>{localEndpoint ? "local" : "remote"}</span>
        </div>

        {#if ai.loading}
          <p class="state mono faint">reading providers and keyring…</p>
        {:else if ai.providers.length === 0}
          <p class="state faint">No providers are available.</p>
        {:else}
          <label class="label" for="ai-provider">Provider</label>
          <select
            id="ai-provider"
            class="field"
            value={ai.providerId}
            disabled={ai.streaming}
            onchange={(event) => selectProvider(event.currentTarget.value)}
          >
            {#each ai.providers as item (item.id)}
              <option value={item.id}>{item.label}</option>
            {/each}
          </select>

          <label class="label" for="ai-model">Model</label>
          <input
            id="ai-model"
            class="field mono"
            list="ai-models"
            value={ai.model}
            disabled={ai.streaming}
            oninput={(event) => setModel(event.currentTarget.value)}
            placeholder="model identifier"
          />
          <datalist id="ai-models">
            {#each current?.models ?? [] as model (model)}
              <option value={model}></option>
            {/each}
          </datalist>

          <label class="label" for="ai-endpoint">Endpoint</label>
          <input
            id="ai-endpoint"
            class="field mono"
            value={ai.endpoint}
            disabled={ai.streaming}
            oninput={(event) => setEndpoint(event.currentTarget.value)}
            placeholder={current?.defaultEndpoint ?? "https://…"}
          />
          <p class="hint">{current?.hint}</p>

          <div class="key-head">
            <span class="label">API key</span>
            <span class="badge" class:ok={status?.configured} class:warn={!status?.configured && current?.keyRequired}>
              {status?.configured ? "stored" : current?.keyRequired ? "required" : "optional"}
            </span>
          </div>
          <div class="key-row">
            <input
              class="field mono"
              type={showKey ? "text" : "password"}
              bind:value={keyInput}
              placeholder={status?.configured ? "replace stored key" : "paste key"}
              autocomplete="off"
              spellcheck="false"
            />
            <button class="btn ghost icon" title={showKey ? "Hide key" : "Show key"} onclick={() => (showKey = !showKey)}>
              {showKey ? "◉" : "○"}
            </button>
          </div>
          <div class="key-actions">
            <button class="btn" onclick={saveProviderKey} disabled={!keyInput.trim()}>Save key</button>
            {#if status?.configured}
              <button class="btn ghost" onclick={removeProviderKey}>Remove</button>
            {/if}
          </div>
          <p class="hint">Keys live in the system keyring — never SQLite, logs or webview storage.</p>
          {#if keyError()}
            <p class="error">{keyError()}</p>
          {/if}

          <label class="label" for="ai-system">System prompt <span class="faint">optional</span></label>
          <textarea
            id="ai-system"
            class="field system"
            rows="3"
            value={ai.system}
            disabled={ai.streaming}
            oninput={(event) => setSystem(event.currentTarget.value)}
            placeholder="How should the assistant behave?"
          ></textarea>
        {/if}
      </section>

      <section class="panel context-panel">
        <div class="panel-head">
          <span class="mono-label">context</span>
          <span class="badge">{ai.context.length} attached</span>
        </div>
        <p class="hint">
          Nothing is attached automatically. Choose a project and the exact files to share
          with this message.
        </p>

        <label class="label" for="ai-project">Project</label>
        <select
          id="ai-project"
          class="field"
          value={ai.projectId ?? ""}
          disabled={ai.streaming || projectsLoading}
          onchange={(event) => chooseProject(event.currentTarget.value)}
        >
          <option value="">{projectsLoading ? "loading projects…" : "No project"}</option>
          {#each projects as project (project.id)}
            <option value={project.id}>{project.name}</option>
          {/each}
        </select>

        {#if ai.projectId !== null}
          <input
            class="field filter"
            bind:value={fileFilter}
            placeholder="Filter files…"
            spellcheck="false"
          />
          <div class="file-list">
            {#if ai.contextLoading}
              <p class="state mono faint">scanning files…</p>
            {:else if ai.contextFiles.length === 0}
              <p class="state faint">No readable text files in this project.</p>
            {:else if filteredFiles.length === 0}
              <p class="state faint">No files match “{fileFilter}”.</p>
            {:else}
              {#each filteredFiles as file (file.path)}
                <button
                  class="file"
                  class:attached={isAttached(file.path)}
                  onclick={() => attachFile(file.path)}
                  disabled={ai.streaming}
                  title={file.path}
                >
                  <span class="file-path mono">{file.path}</span>
                  <span class="file-size mono faint">{size(file.bytes)}</span>
                  <span class="file-mark mono">{isAttached(file.path) ? "✓" : "+"}</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}

        {#if ai.context.length > 0}
          <div class="attached">
            {#each ai.context as item (item.path)}
              <span class="chip" title={item.truncated ? "Truncated to the context limit" : item.path}>
                <span class="mono">{item.path}</span>
                {#if item.source === "view"}<span class="badge">view</span>{/if}
                {#if item.truncated}<span class="badge warn">cut</span>{/if}
                <button class="chip-x" title="Remove context" onclick={() => removeContext(item.path)} disabled={ai.streaming}>×</button>
              </span>
            {/each}
            <button class="btn ghost clear" onclick={clearContext} disabled={ai.streaming}>Clear context</button>
          </div>
        {/if}

        {#if consentNeeded && current}
          <label class="consent">
            <input
              type="checkbox"
              checked={ai.consent}
              onchange={(event) => setConsent(event.currentTarget.checked)}
              disabled={ai.streaming}
            />
            <span>
              I approve sending {ai.context.length} selected {ai.context.length === 1 ? "item" : "items"}
              to <span class="mono">{current.label}</span>.
            </span>
          </label>
        {/if}
      </section>

      <section class="panel conversations">
        <div class="panel-head">
          <span class="mono-label">conversations</span>
          <span class="badge">{ai.conversations.length}</span>
        </div>
        {#if ai.conversationsLoading}
          <p class="state mono faint">loading…</p>
        {:else if ai.conversations.length === 0}
          <p class="state faint">No saved conversations yet.</p>
        {:else}
          <div class="conversation-list">
            {#each ai.conversations as conversation (conversation.id)}
              <div class="conversation" class:active={ai.conversationId === conversation.id}>
                <button
                  class="conversation-open"
                  onclick={() => openConversation(conversation.id)}
                  disabled={ai.streaming || ai.messageLoading}
                >
                  <span class="conversation-title">{conversation.title}</span>
                  <span class="conversation-meta mono faint">
                    {conversation.messageCount} msg · {relative(conversation.updatedAt)}
                  </span>
                </button>
                <button
                  class="conversation-delete"
                  title="Delete conversation"
                  onclick={() => removeConversation(conversation.id, conversation.title)}
                  disabled={ai.streaming}
                >×</button>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </aside>

    <section class="panel chat">
      <div class="chat-head">
        <div class="chat-title">
          <span class="title">{ai.conversations.find((item) => item.id === ai.conversationId)?.title ?? "New conversation"}</span>
          <span class="mono faint">
            {current?.label ?? "no provider"} · {ai.model || "no model"}
          </span>
        </div>
        <div class="chat-flags">
          {#if ai.context.length > 0}
            <span class="badge accent">{ai.context.length} context</span>
          {/if}
          <span class="badge" class:warn={ai.error}>{ai.error ? "attention" : "ready"}</span>
        </div>
      </div>

      <div class="messages" bind:this={messagesEl} onscroll={onMessagesScroll}>
        {#if ai.messageLoading}
          <p class="empty mono faint">opening conversation…</p>
        {:else if ai.messages.length === 0 && !ai.streaming}
          <div class="empty">
            <p class="empty-title">Start with a question, a review or an extraction.</p>
            <p class="empty-copy">
              Attach only the files you choose. Every request shows which provider will receive
              the selected context before you send it.
            </p>
          </div>
        {:else}
          {#each ai.messages as item (item.id)}
            <article class="message" class:user={item.role === "user"} class:assistant={item.role === "assistant"}>
              <header class="message-head">
                <span class="mono-label">{item.role === "user" ? "you" : "assistant"}</span>
                {#if item.createdAt > 0}
                  <span class="mono faint">{relative(item.createdAt)}</span>
                {/if}
              </header>
              {#if item.role === "assistant"}
                <div class="markdown">{@html render(item.content)}</div>
              {:else}
                <p class="user-text">{item.content}</p>
              {/if}
              {#if item.context.length > 0}
                <p class="context-note mono faint">context: {item.context.join(" · ")}</p>
              {/if}
            </article>
          {/each}
        {/if}

        {#if ai.streaming}
          <article class="message assistant streaming">
            <header class="message-head">
              <span class="mono-label">assistant</span>
              <span class="mono faint">{ai.cancelling ? "stopping…" : "streaming…"}</span>
            </header>
            {#if ai.streamText}
              <div class="markdown">{@html render(ai.streamText)}</div>
            {:else}
              <p class="thinking mono faint">waiting for the first token…</p>
            {/if}
          </article>
        {/if}
      </div>

      {#if ai.error}
        <p class="error chat-error">{ai.error}</p>
      {/if}

      <div class="composer">
        <textarea
          class="field composer-input"
          rows="3"
          bind:value={ai.draft}
          onkeydown={onComposerKeydown}
          disabled={ai.streaming || ai.messageLoading}
          placeholder={current?.keyRequired && !keyConfigured()
            ? "Add a provider key to start…"
            : "Ask about the attached context, or start without any…"}
        ></textarea>
        <div class="composer-foot">
          <div class="composer-meta mono faint">
            <span>{ai.context.length} context</span>
            <span>·</span>
            <span>{ai.draft.length} chars</span>
            <span>·</span>
            <span>Enter to send · Shift+Enter for a new line</span>
          </div>
          {#if ai.streaming}
            <button class="btn" onclick={cancelMessage} disabled={ai.cancelling}>
              {ai.cancelling ? "Stopping…" : "Stop"}
            </button>
          {:else}
            <button class="btn primary" onclick={send} disabled={!sendable}>Send</button>
          {/if}
        </div>
        {#if ai.context.length > 0 && current}
          <p class="privacy mono faint">
            {localEndpoint ? "Local endpoint" : "Remote endpoint"}: {current.label} · {ai.context.length}
            selected {ai.context.length === 1 ? "item" : "items"} will leave GRAFT only when you send.
          </p>
        {/if}
      </div>
    </section>
  </div>
</section>

<style>
  .ai-view {
    max-width: 1500px;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ai-grid {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    gap: 12px;
    align-items: start;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-bottom: 9px;
    margin-bottom: 9px;
    border-bottom: 1px solid var(--border);
  }

  .provider-panel,
  .context-panel,
  .conversations {
    padding: 11px 12px 12px;
  }

  .label {
    display: block;
    margin: 9px 0 4px;
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .label:first-of-type {
    margin-top: 0;
  }

  .hint {
    margin-top: 6px;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .state {
    padding: 8px 0;
    font-size: 11px;
  }

  .key-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .key-head .label {
    margin: 0;
  }

  .key-row {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  .key-row .field {
    min-width: 0;
  }

  .key-actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .system {
    min-height: 58px;
    resize: vertical;
  }

  .filter {
    margin-top: 7px;
  }

  .file-list {
    max-height: 210px;
    margin-top: 6px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .file {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 12px;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 4px 7px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .file:last-child {
    border-bottom: 0;
  }

  .file:hover:not(:disabled) {
    background: var(--surface-alt);
  }

  .file.attached {
    background: var(--accent-soft);
  }

  .file:disabled {
    cursor: default;
  }

  .file-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .file-size {
    font-size: 9px;
  }

  .file-mark {
    color: var(--text-faint);
    font-size: 10px;
  }

  .file.attached .file-mark {
    color: var(--accent);
  }

  .attached {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 8px;
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    padding: 3px 5px 3px 7px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: var(--bg-elev);
  }

  .chip > .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    color: var(--text-muted);
  }

  .chip-x {
    margin-left: auto;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    line-height: 1;
  }

  .chip-x:hover {
    color: var(--danger);
  }

  .clear {
    align-self: flex-start;
  }

  .consent {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    margin-top: 9px;
    padding: 8px;
    border: 1px solid color-mix(in srgb, var(--warn) 35%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warn) 8%, transparent);
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-muted);
    cursor: pointer;
  }

  .consent input {
    margin-top: 2px;
    accent-color: var(--warn);
  }

  .conversation-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 190px;
    overflow: auto;
  }

  .conversation {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: 2px;
  }

  .conversation:hover,
  .conversation.active {
    background: var(--surface-alt);
  }

  .conversation.active {
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .conversation-open {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    min-width: 0;
    flex: 1;
    padding: 5px 6px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .conversation-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
    font-size: 11px;
    color: var(--text);
  }

  .conversation-meta {
    font-size: 9px;
  }

  .conversation-delete {
    flex: 0 0 auto;
    padding: 4px 6px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    line-height: 1;
  }

  .conversation-delete:hover {
    color: var(--danger);
  }

  .chat {
    display: flex;
    flex-direction: column;
    min-height: calc(100vh - 170px);
    max-height: calc(100vh - 170px);
    overflow: hidden;
  }

  .chat-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 13px;
    border-bottom: 1px solid var(--border);
  }

  .chat-title {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .chat-title .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-weight: 600;
  }

  .chat-flags {
    display: flex;
    gap: 6px;
    flex: 0 0 auto;
  }

  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .empty {
    margin: auto;
    max-width: 46ch;
    text-align: center;
    padding: 22px;
  }

  .empty-title {
    font-size: 12.5px;
    color: var(--text);
  }

  .empty-copy {
    margin-top: 5px;
    font-size: 11px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .message {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-width: 88%;
  }

  .message.user {
    align-self: flex-end;
    align-items: flex-end;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-alt);
  }

  .message.assistant {
    align-self: flex-start;
    width: 100%;
    max-width: 100%;
  }

  .message-head {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .message.user .message-head {
    justify-content: flex-end;
  }

  .user-text {
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .markdown {
    font-size: 12px;
    line-height: 1.62;
    color: var(--text);
    user-select: text;
    word-break: break-word;
  }

  .markdown :global(p) {
    margin: 0 0 7px;
  }

  .markdown :global(p:last-child) {
    margin-bottom: 0;
  }

  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3),
  .markdown :global(h4) {
    margin: 12px 0 5px;
    font-size: 13px;
    font-weight: 600;
  }

  .markdown :global(ul),
  .markdown :global(ol) {
    margin: 5px 0 8px;
    padding-left: 18px;
    list-style: disc;
  }

  .markdown :global(ol) {
    list-style: decimal;
  }

  .markdown :global(li) {
    margin: 2px 0;
  }

  .markdown :global(code) {
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 2px;
    background: var(--bg-elev);
    font-family: var(--font-mono);
    font-size: 10.5px;
  }

  .markdown :global(pre) {
    margin: 7px 0;
    padding: 8px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-elev);
  }

  .markdown :global(pre code) {
    padding: 0;
    border: 0;
    background: transparent;
  }

  .markdown :global(blockquote) {
    margin: 6px 0;
    padding-left: 9px;
    border-left: 2px solid var(--border-strong);
    color: var(--text-muted);
  }

  .markdown :global(table) {
    border-collapse: collapse;
    margin: 7px 0;
    font-size: 11px;
  }

  .markdown :global(th),
  .markdown :global(td) {
    padding: 3px 7px;
    border: 1px solid var(--border);
    text-align: left;
  }

  .markdown :global(a) {
    color: var(--info);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .context-note {
    font-size: 9px;
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .streaming {
    opacity: 0.95;
  }

  .thinking {
    font-size: 10.5px;
  }

  .composer {
    padding: 10px 12px 11px;
    border-top: 1px solid var(--border);
  }

  .composer-input {
    min-height: 72px;
    resize: vertical;
    font-size: 12px;
  }

  .composer-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 7px;
  }

  .composer-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
    font-size: 9px;
  }

  .privacy {
    margin-top: 7px;
    font-size: 9px;
    line-height: 1.4;
  }

  .error {
    color: var(--danger);
    font-size: 10.5px;
    line-height: 1.5;
  }

  .chat-error {
    margin: 0 12px 8px;
    padding: 7px 9px;
    border: 1px solid color-mix(in srgb, var(--danger) 40%, var(--border));
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--danger) 8%, transparent);
  }

  @media (max-width: 980px) {
    .ai-grid {
      grid-template-columns: 1fr;
    }

    .side {
      order: 2;
    }

    .chat {
      order: 1;
      min-height: 64vh;
      max-height: 72vh;
    }
  }
</style>
