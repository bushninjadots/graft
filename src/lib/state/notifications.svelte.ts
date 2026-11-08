import { api, type GithubRepo } from "../api";
import { github } from "./github.svelte";
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
const LIMIT = 80;

export const notifications = $state<NotificationState>({
  loaded: false,
  items: [],
});

let writeChain: Promise<void> = Promise.resolve();

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

function id(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `n-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

/** Records a persistent notification and shows a brief clickable toast. */
export function pushNotification(input: {
  title: string;
  detail?: string;
  tone?: "neutral" | "ok" | "danger";
  target: NotificationTarget;
}): AppNotification {
  const item: AppNotification = {
    id: id(),
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
