import {
  api,
  type GithubRepo,
  type VisitedRepo,
  type WatchtowerEvent,
  type WatchtowerRecommendation,
  type WatchtowerRepo,
  type WatchtowerSnapshot,
} from "../api";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { isSignedIn } from "./github.svelte";
import { notify } from "./ui.svelte";
import { pushNotification } from "./notifications.svelte";

export type WatchCategory =
  | "commits"
  | "pull_requests"
  | "issues"
  | "releases"
  | "reviews"
  | "workflows"
  | "stars"
  | "forks";

export const WATCH_CATEGORIES: { id: WatchCategory; label: string }[] = [
  { id: "commits", label: "Commits" },
  { id: "pull_requests", label: "Pull requests" },
  { id: "issues", label: "Issues" },
  { id: "releases", label: "Releases" },
  { id: "reviews", label: "Reviews" },
  { id: "workflows", label: "Workflow failures" },
  { id: "stars", label: "Stars" },
  { id: "forks", label: "Forks" },
];

const SETTINGS_KEY = "watchtower.settings";
const ALL_CATEGORIES = WATCH_CATEGORIES.map((category) => category.id);
const POLL_INTERVAL = 180_000;

type WatchtowerSettings = {
  enabled: boolean;
  native: boolean;
  categories: WatchCategory[];
};

type WatchtowerState = {
  loaded: boolean;
  loading: boolean;
  error: string | null;
  repos: WatchtowerRepo[];
  events: WatchtowerEvent[];
  unread: number;
  enabled: boolean;
  native: boolean;
  categories: WatchCategory[];
  polling: boolean;
  lastPollAt: number | null;
  lastPollNotModified: number;
  starred: GithubRepo[];
  visited: VisitedRepo[];
  recommended: WatchtowerRecommendation[];
  discoveryLoaded: boolean;
  discoveryLoading: boolean;
  discoveryError: string | null;
};

export const watchtower = $state<WatchtowerState>({
  loaded: false,
  loading: false,
  error: null,
  repos: [],
  events: [],
  unread: 0,
  enabled: false,
  native: true,
  categories: [...ALL_CATEGORIES],
  polling: false,
  lastPollAt: null,
  lastPollNotModified: 0,
  starred: [],
  visited: [],
  recommended: [],
  discoveryLoaded: false,
  discoveryLoading: false,
  discoveryError: null,
});

let persistTimer: ReturnType<typeof setTimeout> | undefined;
let polling = false;

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function normalizeCategories(value: unknown): WatchCategory[] {
  if (!Array.isArray(value)) return [...ALL_CATEGORIES];
  const valid = new Set<string>(ALL_CATEGORIES);
  const kept = value.filter(
    (item): item is WatchCategory => typeof item === "string" && valid.has(item),
  );
  return kept.length > 0 ? [...new Set(kept)] : [...ALL_CATEGORIES];
}

function persist(): void {
  const payload: WatchtowerSettings = {
    enabled: watchtower.enabled,
    native: watchtower.native,
    categories: watchtower.categories,
  };
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    void api
      .setSetting(SETTINGS_KEY, JSON.stringify(payload))
      .catch(() => undefined);
  }, 180);
}

function applySnapshot(snapshot: WatchtowerSnapshot): void {
  watchtower.repos = snapshot.repos;
  watchtower.events = snapshot.events;
  watchtower.unread = snapshot.unread;
}

export async function loadWatchtower(force = false): Promise<void> {
  if (watchtower.loading || (watchtower.loaded && !force)) return;
  watchtower.loading = true;
  watchtower.error = null;
  try {
    const [raw, snapshot] = await Promise.all([
      api.getSetting(SETTINGS_KEY).catch(() => null),
      api.watchtowerSnapshot(),
    ]);
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as Partial<WatchtowerSettings>;
        watchtower.enabled = parsed.enabled === true;
        watchtower.native = parsed.native !== false;
        watchtower.categories = normalizeCategories(parsed.categories);
      } catch {
        watchtower.enabled = false;
        watchtower.native = true;
        watchtower.categories = [...ALL_CATEGORIES];
      }
    }
    applySnapshot(snapshot);
    const latest = snapshot.repos
      .map((repo) => repo.lastPolledAt ?? 0)
      .reduce((max, value) => Math.max(max, value), 0);
    watchtower.lastPollAt = latest > 0 ? latest * 1000 : null;
    watchtower.loaded = true;
  } catch (cause) {
    watchtower.error = message(cause);
  } finally {
    watchtower.loading = false;
  }
}

export async function refreshWatchtower(): Promise<void> {
  try {
    applySnapshot(await api.watchtowerSnapshot());
    watchtower.error = null;
  } catch (cause) {
    watchtower.error = message(cause);
  }
}

/**
 * Loads real stars from GitHub, local visit history and derived similar
 * repositories. This is view-scoped so boot and dashboard reads stay cheap.
 */
export async function loadWatchtowerDiscovery(force = false): Promise<void> {
  if (watchtower.discoveryLoading) return;
  if (watchtower.discoveryLoaded && !force) return;
  if (!isSignedIn()) return;
  watchtower.discoveryLoading = true;
  try {
    const discovery = await api.watchtowerDiscover();
    watchtower.starred = discovery.starred;
    watchtower.visited = discovery.visited;
    watchtower.recommended = discovery.recommended;
    watchtower.discoveryError =
      discovery.errors.length > 0 ? discovery.errors.join(" · ") : null;
    watchtower.discoveryLoaded = true;
  } catch (cause) {
    watchtower.discoveryError = message(cause);
  } finally {
    watchtower.discoveryLoading = false;
  }
}

export function setWatchtowerEnabled(enabled: boolean): void {
  watchtower.enabled = enabled;
  persist();
  if (enabled) void pollWatchtower(true);
}

export function setNativeAlerts(enabled: boolean): void {
  watchtower.native = enabled;
  persist();
  if (enabled) void ensureNativePermission();
}

export function toggleWatchCategory(category: WatchCategory): void {
  const active = watchtower.categories.includes(category);
  if (active && watchtower.categories.length === 1) {
    notify("Keep at least one alert category enabled.", "danger");
    return;
  }
  watchtower.categories = active
    ? watchtower.categories.filter((item) => item !== category)
    : [...watchtower.categories, category];
  persist();
}

export function isWatched(fullName: string): boolean {
  return watchtower.repos.some(
    (repo) => repo.fullName.toLowerCase() === fullName.toLowerCase(),
  );
}

export async function addWatch(owner: string, repo: string): Promise<boolean> {
  const cleanOwner = owner.trim();
  const cleanRepo = repo.trim();
  if (!cleanOwner || !cleanRepo) {
    notify("Enter a repository as owner/name.", "danger");
    return false;
  }
  try {
    const saved = await api.watchRepo(cleanOwner, cleanRepo);
    await refreshWatchtower();
    await loadWatchtowerDiscovery(true);
    notify(`Watching ${saved.fullName}`, "ok");
    return true;
  } catch (cause) {
    notify(message(cause), "danger");
    return false;
  }
}

export async function addWatchByFullName(fullName: string): Promise<boolean> {
  const [owner, repo, extra] = fullName.trim().split("/");
  if (!owner || !repo || extra) {
    notify("Enter a repository as owner/name.", "danger");
    return false;
  }
  return addWatch(owner, repo);
}

export async function removeWatch(repo: WatchtowerRepo): Promise<void> {
  try {
    await api.unwatchRepo(repo.id);
    await refreshWatchtower();
    await loadWatchtowerDiscovery(true);
    notify(`Stopped watching ${repo.fullName}`, "ok");
  } catch (cause) {
    notify(message(cause), "danger");
  }
}

export async function markWatchtowerRead(id: number | null): Promise<void> {
  try {
    watchtower.unread = await api.markWatchtowerRead(id);
    watchtower.events = watchtower.events.map((event) =>
      id === null || event.id === id ? { ...event, seen: true } : event,
    );
  } catch (cause) {
    notify(message(cause), "danger");
  }
}

async function ensureNativePermission(): Promise<boolean> {
  try {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  } catch {
    return false;
  }
}

async function sendNative(title: string, body: string): Promise<void> {
  if (!watchtower.native || document.visibilityState === "visible") return;
  if (!(await ensureNativePermission())) return;
  try {
    sendNotification({ title, body });
  } catch {
    // A missing desktop notification daemon must not break polling.
  }
}

function eventNotification(event: WatchtowerEvent): void {
  const label =
    WATCH_CATEGORIES.find((category) => category.id === event.category)?.label ??
    "Repository activity";
  pushNotification({
    id: `watch:${event.id}`,
    title: `${label} · ${event.repoFullName}`,
    detail: event.title,
    tone: event.category === "workflows" ? "danger" : "neutral",
    target: { view: "watchtower" },
  });
}

/**
 * Polls the favourites through the Rust engine. Background polling only runs
 * when monitoring is enabled and is throttled; the manual Check now button
 * bypasses both the switch and the throttle.
 */
export async function pollWatchtower(force = false): Promise<void> {
  if (polling || !isSignedIn()) return;
  if (!force) {
    if (!watchtower.enabled) return;
    if (watchtower.lastPollAt && Date.now() - watchtower.lastPollAt < POLL_INTERVAL) {
      return;
    }
  }
  polling = true;
  watchtower.polling = true;
  try {
    const result = await api.pollWatchtower(watchtower.categories);
    watchtower.lastPollAt = Date.now();
    watchtower.lastPollNotModified = result.notModified;
    if (result.errors.length > 0 && force) {
      notify(result.errors[0], "danger");
    }
    for (const event of result.newEvents) eventNotification(event);
    if (result.newEvents.length > 0) {
      const first = result.newEvents[0];
      void sendNative(
        result.newEvents.length === 1
          ? `Watchtower · ${first.repoFullName}`
          : `${result.newEvents.length} Watchtower alerts`,
        result.newEvents.length === 1
          ? first.title
          : result.newEvents
              .slice(0, 3)
              .map((event) => `${event.repoFullName}: ${event.title}`)
              .join("\n"),
      );
    }
    if (result.checked > 0 && result.newEvents.length === 0 && force) {
      notify(
        result.notModified > 0
          ? `No new activity · ${result.notModified} conditional request${result.notModified === 1 ? "" : "s"} returned 304`
          : "No new activity in your watched repositories",
        "ok",
      );
    }
    await refreshWatchtower();
  } catch (cause) {
    if (force) notify(message(cause), "danger");
  } finally {
    polling = false;
    watchtower.polling = false;
  }
}
