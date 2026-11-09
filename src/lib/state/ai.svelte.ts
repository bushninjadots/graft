import {
  api,
  type AiCatalog,
  type AiContextFile,
  type AiContextInput,
  type AiContextItem,
  type AiConversation,
  type AiMessage,
  type AiMessageInput,
  type AiProvider,
} from "../api";
import { notify } from "./ui.svelte";

const PREFERENCES_KEY = "ai.preferences";
const MAX_ATTACHMENTS = 12;
const MAX_CONTEXT_ITEM_CHARS = 60_000;
const MAX_CONTEXT_TOTAL_CHARS = 240_000;

export type AttachedContext = AiContextItem & {
  source: "project" | "view";
};

type AiState = {
  loaded: boolean;
  loading: boolean;
  error: string | null;
  providers: AiProvider[];
  keys: Record<string, { configured: boolean; error: string | null }>;
  providerId: string;
  model: string;
  endpoint: string;
  system: string;
  conversations: AiConversation[];
  conversationsLoading: boolean;
  conversationId: number | null;
  messages: AiMessage[];
  messageLoading: boolean;
  draft: string;
  context: AttachedContext[];
  projectId: number | null;
  contextFiles: AiContextFile[];
  contextLoading: boolean;
  consent: boolean;
  streaming: boolean;
  streamText: string;
  requestId: string;
  cancelling: boolean;
};

export const ai = $state<AiState>({
  loaded: false,
  loading: false,
  error: null,
  providers: [],
  keys: {},
  providerId: "anthropic",
  model: "",
  endpoint: "",
  system: "",
  conversations: [],
  conversationsLoading: false,
  conversationId: null,
  messages: [],
  messageLoading: false,
  draft: "",
  context: [],
  projectId: null,
  contextFiles: [],
  contextLoading: false,
  consent: false,
  streaming: false,
  streamText: "",
  requestId: "",
  cancelling: false,
});

let preferencesTimer: ReturnType<typeof setTimeout> | undefined;

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

export function provider(): AiProvider | undefined {
  return ai.providers.find((item) => item.id === ai.providerId);
}

export function keyConfigured(id = ai.providerId): boolean {
  return ai.keys[id]?.configured ?? false;
}

export function keyError(id = ai.providerId): string | null {
  return ai.keys[id]?.error ?? null;
}

export function usesLocalEndpoint(): boolean {
  const raw = ai.endpoint.trim();
  if (!raw) return provider()?.local ?? false;
  try {
    const url = new URL(raw);
    const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, "");
    return (
      host === "localhost" ||
      host === "::1" ||
      /^127\./.test(host)
    );
  } catch {
    return false;
  }
}

export function needsConsent(): boolean {
  return ai.context.length > 0 && !usesLocalEndpoint();
}

export function canSend(): boolean {
  if (ai.streaming || ai.messageLoading) return false;
  if (!provider() || !ai.model.trim()) return false;
  if (ai.draft.trim().length === 0) return false;
  if (!keyConfigured() && provider()?.keyRequired) return false;
  if (needsConsent() && !ai.consent) return false;
  return true;
}

function persistPreferences(): void {
  if (preferencesTimer) clearTimeout(preferencesTimer);
  preferencesTimer = setTimeout(() => {
    const value = JSON.stringify({
      providerId: ai.providerId,
      model: ai.model,
      endpoint: ai.endpoint,
      system: ai.system,
    });
    void api.setSetting(PREFERENCES_KEY, value).catch(() => undefined);
  }, 450);
}

async function loadPreferences(): Promise<void> {
  try {
    const raw = await api.getSetting(PREFERENCES_KEY);
    if (!raw) return;
    const saved = JSON.parse(raw) as Partial<{
      providerId: string;
      model: string;
      endpoint: string;
      system: string;
    }>;
    if (saved.providerId) ai.providerId = saved.providerId;
    if (saved.model) ai.model = saved.model;
    if (saved.endpoint) ai.endpoint = saved.endpoint;
    if (saved.system) ai.system = saved.system;
  } catch {
    // A corrupt preference blob falls back to provider defaults.
  }
}

function applyProviderDefaults(id: string): void {
  const next = ai.providers.find((item) => item.id === id);
  if (!next) return;
  ai.model = next.models[0] ?? "";
  ai.endpoint = next.defaultEndpoint;
}

export async function loadAI(): Promise<void> {
  if (ai.loading) return;
  ai.loading = true;
  ai.error = null;
  try {
    const [catalog] = await Promise.all([api.aiCatalog(), loadPreferences()]);
    applyCatalog(catalog);
    await loadConversations();
    ai.loaded = true;
  } catch (cause) {
    ai.error = message(cause);
  } finally {
    ai.loading = false;
  }
}

function applyCatalog(catalog: AiCatalog): void {
  ai.providers = catalog.providers;
  ai.keys = Object.fromEntries(
    catalog.keys.map((status) => [
      status.provider,
      { configured: status.configured, error: status.error },
    ]),
  );
  if (!ai.providers.some((item) => item.id === ai.providerId)) {
    ai.providerId = ai.providers[0]?.id ?? "anthropic";
    applyProviderDefaults(ai.providerId);
  } else if (!ai.model) {
    applyProviderDefaults(ai.providerId);
  }
  if (!ai.endpoint) {
    ai.endpoint = provider()?.defaultEndpoint ?? "";
  }
}

export async function refreshCatalog(): Promise<void> {
  try {
    applyCatalog(await api.aiCatalog());
  } catch (cause) {
    ai.error = message(cause);
  }
}

export function selectProvider(id: string): void {
  if (ai.streaming || id === ai.providerId) return;
  ai.providerId = id;
  applyProviderDefaults(id);
  ai.consent = false;
  persistPreferences();
}

export function setModel(value: string): void {
  ai.model = value;
  persistPreferences();
}

export function setEndpoint(value: string): void {
  ai.endpoint = value;
  ai.consent = false;
  persistPreferences();
}

export function setSystem(value: string): void {
  ai.system = value;
  persistPreferences();
}

export async function saveKey(key: string): Promise<boolean> {
  if (!key.trim()) {
    ai.error = "Paste a key before saving.";
    return false;
  }
  try {
    await api.aiSaveKey(ai.providerId, key);
    await refreshCatalog();
    notify(`Key saved for ${provider()?.label ?? ai.providerId}.`, "ok");
    return true;
  } catch (cause) {
    ai.error = message(cause);
    return false;
  }
}

export async function clearKey(): Promise<void> {
  try {
    await api.aiClearKey(ai.providerId);
    await refreshCatalog();
    notify("Provider key removed from the system keyring.", "ok");
  } catch (cause) {
    ai.error = message(cause);
  }
}

export async function loadConversations(): Promise<void> {
  ai.conversationsLoading = true;
  try {
    ai.conversations = await api.aiConversations();
  } catch (cause) {
    ai.error = message(cause);
  } finally {
    ai.conversationsLoading = false;
  }
}

export async function openConversation(id: number): Promise<void> {
  if (ai.streaming) return;
  ai.messageLoading = true;
  ai.error = null;
  try {
    const detail = await api.aiConversation(id);
    ai.conversationId = detail.conversation.id;
    ai.messages = detail.messages;
    ai.streamText = "";
    const known = ai.providers.some((item) => item.id === detail.conversation.provider);
    if (known) {
      ai.providerId = detail.conversation.provider;
      ai.model = detail.conversation.model;
      ai.endpoint = detail.conversation.endpoint || provider()?.defaultEndpoint || ai.endpoint;
      ai.consent = false;
    }
  } catch (cause) {
    ai.error = message(cause);
  } finally {
    ai.messageLoading = false;
  }
}

export function newConversation(): void {
  if (ai.streaming) return;
  ai.conversationId = null;
  ai.messages = [];
  ai.streamText = "";
  ai.draft = "";
  ai.error = null;
  ai.consent = false;
}

export async function deleteConversation(id: number): Promise<void> {
  try {
    await api.aiDeleteConversation(id);
    if (ai.conversationId === id) newConversation();
    await loadConversations();
  } catch (cause) {
    ai.error = message(cause);
  }
}

export function setContextProject(id: number | null): void {
  ai.projectId = id;
  ai.contextFiles = [];
  ai.context = ai.context.filter((item) => item.source !== "project");
  ai.consent = false;
}

export async function loadContextFiles(): Promise<void> {
  if (ai.projectId === null) {
    ai.contextFiles = [];
    return;
  }
  ai.contextLoading = true;
  try {
    ai.contextFiles = await api.aiContextScan(ai.projectId);
  } catch (cause) {
    ai.contextFiles = [];
    ai.error = message(cause);
  } finally {
    ai.contextLoading = false;
  }
}

export function isAttached(path: string): boolean {
  return ai.context.some((item) => item.path === path);
}

export async function attachFile(path: string): Promise<void> {
  if (ai.projectId === null || ai.streaming) return;
  if (isAttached(path)) return;
  if (ai.context.length >= MAX_ATTACHMENTS) {
    ai.error = `Attach at most ${MAX_ATTACHMENTS} context items to one message.`;
    return;
  }
  try {
    const [item] = await api.aiContextRead(ai.projectId, [path]);
    if (!item) return;
    ai.context = [...ai.context, { ...item, source: "project" }];
    ai.consent = false;
  } catch (cause) {
    ai.error = message(cause);
  }
}

export function removeContext(path: string): void {
  ai.context = ai.context.filter((item) => item.path !== path);
  ai.consent = false;
}

export function clearContext(): void {
  ai.context = [];
  ai.consent = false;
}

export function setConsent(value: boolean): void {
  ai.consent = value;
}

/** Seeds a new conversation from another surface (review or extraction). */
export function focusContext(items: AiContextInput[], prompt: string): void {
  if (ai.streaming) {
    ai.error = "Stop the current reply before starting a new request.";
    return;
  }
  newConversation();
  let total = 0;
  const attached: AttachedContext[] = [];
  for (const item of items.slice(0, MAX_ATTACHMENTS)) {
    if (total >= MAX_CONTEXT_TOTAL_CHARS) break;
    const limit = Math.min(MAX_CONTEXT_ITEM_CHARS, MAX_CONTEXT_TOTAL_CHARS - total);
    const content = item.content.slice(0, limit);
    attached.push({
      path: item.title,
      content,
      truncated: content.length < item.content.length,
      source: "view",
    });
    total += content.length;
  }
  ai.context = attached;
  ai.draft = prompt;
  ai.consent = false;
}

function requestMessages(): AiMessageInput[] {
  const history: AiMessageInput[] = ai.messages
    .filter((item) => item.role === "user" || item.role === "assistant")
    .map((item) => ({ role: item.role, content: item.content }));
  history.push({ role: "user", content: ai.draft.trim() });
  return history;
}

export async function sendMessage(): Promise<void> {
  if (!canSend()) return;
  const current = provider();
  if (!current) {
    ai.error = "Choose an AI provider first.";
    return;
  }
  const text = ai.draft.trim();
  const messages = requestMessages();
  const context = ai.context.map((item) => ({
    title: item.path,
    content: item.content,
  }));
  const requestId =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  ai.requestId = requestId;
  ai.streaming = true;
  ai.cancelling = false;
  ai.streamText = "";
  ai.error = null;
  ai.draft = "";
  ai.messages = [
    ...ai.messages,
    {
      id: -1,
      role: "user",
      content: text,
      context: ai.context.map((item) => item.path),
      createdAt: Math.floor(Date.now() / 1000),
    },
  ];
  const conversationId = ai.conversationId;

  try {
    const reply = await api.aiChat(
      {
        requestId,
        conversationId,
        provider: ai.providerId,
        model: ai.model.trim(),
        endpoint: ai.endpoint.trim() || null,
        system: ai.system.trim() || null,
        messages,
        context,
        consent: ai.consent,
      },
      (event) => {
        if (event.type === "started") {
          ai.conversationId = event.conversationId;
        } else if (event.type === "delta") {
          ai.streamText += event.text;
        }
      },
    );
    ai.conversationId = reply.conversationId;
    ai.streaming = false;
    ai.cancelling = false;
    ai.streamText = "";
    ai.context = [];
    ai.consent = false;
    const detail = await api.aiConversation(reply.conversationId);
    ai.messages = detail.messages;
    await loadConversations();
  } catch (cause) {
    ai.streaming = false;
    ai.cancelling = false;
    ai.streamText = "";
    ai.error = message(cause);
    if (ai.conversationId !== null) {
      try {
        const detail = await api.aiConversation(ai.conversationId);
        ai.messages = detail.messages;
      } catch {
        // Keep the optimistic message visible so the failure is obvious.
      }
      await loadConversations();
    }
  }
}

export async function cancelMessage(): Promise<void> {
  if (!ai.streaming || ai.cancelling) return;
  ai.cancelling = true;
  try {
    await api.aiCancel(ai.requestId);
  } catch (cause) {
    ai.error = message(cause);
    ai.cancelling = false;
  }
}
