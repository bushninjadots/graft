import { api, type GithubNotification, type GithubRepo } from "../api";
import { github, isSignedIn } from "./github.svelte";
import { navigate, notify, ui } from "./ui.svelte";

export type NotificationTarget = {
  view: string;
  repo?: GithubRepo | null;
  tab?: string;
  path?: string;
};

export type AppNotification = {
  id: string;
  at: number;
  title: string;
  detail: string;
  tone: "neutral" | "ok" | "danger";
  read: boolean;
  target: NotificationTarget;
};

type NotificationState = {
  loaded: boolean;
  items: AppNotification[];
};

const KEY = "app.notifications";
const SEEN_KEY = "app.notifications.seen";
const LIMIT = 80;
const MAX_SEEN = 600;

export const notifications = $state<NotificationState>({
  loaded: false,
  items: [],
});

let writeChain: Promise<void> = Promise.resolve();
let seenChain: Promise<void> = Promise.resolve();
let seen = new Set<string>();
let seenLoaded = false;
let polling = false;

function persist(): void {
  const payload = JSON.stringify(notifications.items.slice(0, LIMIT));
  writeChain = writeChain
    .then(() => api.setSetting(KEY, payload))
    .catch(() => undefined);
}

export async function loadNotifications(): Promise<void> {
  if (notifications.loaded) return;
  try {
    const raw = await api.getSetting(KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed)) {
        notifications.items = parsed.filter(isNotification).slice(0, LIMIT);
      }
    }
  } catch {
    notifications.items = [];
  } finally {
    notifications.loaded = true;
  }
}

function isNotification(value: unknown): value is AppNotification {
  if (!value || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  return (
    typeof item.id === "string" &&
    typeof item.title === "string" &&
    typeof item.at === "number" &&
    typeof item.read === "boolean" &&
    !!item.target &&
    typeof (item.target as Record<string, unknown>).view === "string"
  );
}

function nextId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `n-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

function persistSeen(): void {
  const payload = JSON.stringify([...seen].slice(-MAX_SEEN));
  seenChain = seenChain
    .then(() => api.setSetting(SEEN_KEY, payload))
    .catch(() => undefined);
}

async function loadSeen(): Promise<void> {
  if (seenLoaded) return;
  try {
    const raw = await api.getSetting(SEEN_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed)) {
        seen = new Set(parsed.filter((value): value is string => typeof value === "string"));
      }
    }
    for (const item of notifications.items) {
      if (item.id.startsWith("gh:")) seen.add(item.id.slice(3));
    }
  } catch {
    seen = new Set();
  } finally {
    seenLoaded = true;
  }
}

/** Records a persistent notification and shows a brief clickable toast. */
export function pushNotification(input: {
  id?: string;
  title: string;
  detail?: string;
  tone?: "neutral" | "ok" | "danger";
  target: NotificationTarget;
}): AppNotification {
  const item: AppNotification = {
    id: input.id ?? nextId(),
    at: Date.now(),
    title: input.title,
    detail: input.detail ?? "",
    tone: input.tone ?? "neutral",
    read: false,
    target: input.target,
  };
  notifications.items = [item, ...notifications.items].slice(0, LIMIT);
  persist();
  notify(item.title, item.tone, () => openNotification(item));
  return item;
}

export function markRead(notificationId: string): void {
  notifications.items = notifications.items.map((item) =>
    item.id === notificationId ? { ...item, read: true } : item,
  );
  persist();
}

export function markAllRead(): void {
  notifications.items = notifications.items.map((item) => ({ ...item, read: true }));
  persist();
}

export function clearNotifications(): void {
  notifications.items = [];
  persist();
}

export function unreadCount(): number {
  return notifications.items.filter((item) => !item.read).length;
}

function reasonLabel(reason: string): string {
  const labels: Record<string, string> = {
    mention: "You were mentioned",
    team_mention: "Your team was mentioned",
    review_requested: "Review requested",
    assign: "Assigned to you",
    author: "Your thread updated",
    comment: "New comment",
    ci_activity: "Workflow activity",
    state_change: "Thread state changed",
    security_alert: "Security alert",
    invitation: "Repository invitation",
    subscribed: "Repository activity",
  };
  return labels[reason] ?? "GitHub activity";
}

function notificationTab(type: string): string {
  if (type === "PullRequest") return "pulls";
  if (type === "Issue") return "issues";
  return "overview";
}

/**
 * Reads unread GitHub notifications and imports only keys GRAFT has not seen
 * before. It never marks anything read on github.com.
 */
export async function pollGithubNotifications(): Promise<void> {
  if (polling || !isSignedIn()) return;
  polling = true;
  try {
    if (!notifications.loaded) await loadNotifications();
    await loadSeen();
    const external = await api.githubNotifications();
    const fresh = external
      .slice()
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
      .slice(0, 12);
    let changed = false;
    for (const notification of fresh) {
      const key = `${notification.id}:${notification.updatedAt}`;
      if (seen.has(key)) continue;
      seen.add(key);
      changed = true;
      const target = externalTarget(notification);
      if (!target) continue;
      pushNotification({
        id: `gh:${notification.id}:${notification.updatedAt}`,
        title: reasonLabel(notification.reason),
        detail: `${notification.title} · ${notification.repo?.fullName ?? "GitHub"}${
          notification.number ? ` #${notification.number}` : ""
        }`,
        target,
      });
    }
    if (changed) persistSeen();
  } catch {
    // A sync failure must not spam the user or hide local notifications.
  } finally {
    polling = false;
  }
}

function externalTarget(notification: GithubNotification): NotificationTarget | null {
  if (!notification.repo) return null;
  return {
    view: "repository",
    repo: notification.repo,
    tab: notificationTab(notification.subjectType),
  };
}

/** Navigates to the surface that produced the notification. */
export function openNotification(item: AppNotification): void {
  markRead(item.id);
  if (item.target.repo) github.selected = item.target.repo;
  if (item.target.view === "repository") {
    ui.repositoryFocus = {
      tab: item.target.tab ?? "overview",
      path: item.target.path ?? null,
    };
  }
  navigate(item.target.view);
}
