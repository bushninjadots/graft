import { api } from "../api";
import { NAV_ITEMS, defaultNavOrder } from "../nav";

const NAV_KEY = "nav.preferences";

type NavState = {
  order: string[];
  hidden: string[];
  pinned: string[];
};

export const navState = $state<NavState>({
  order: defaultNavOrder(),
  hidden: [],
  pinned: [],
});

let saveTimer: ReturnType<typeof setTimeout> | undefined;

function persist(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api
      .setSetting(
        NAV_KEY,
        JSON.stringify({
          order: navState.order,
          hidden: navState.hidden,
          pinned: navState.pinned,
        }),
      )
      .catch(() => {});
  }, 200);
}

export async function loadNav(): Promise<void> {
  try {
    const raw = await api.getSetting(NAV_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<NavState>;
      const known = new Set(NAV_ITEMS.map((item) => item.id));
      if (Array.isArray(parsed.order)) {
        const kept = parsed.order.filter((id): id is string => typeof id === "string" && known.has(id));
        const missing = NAV_ITEMS.map((item) => item.id).filter((id) => !kept.includes(id));
        navState.order = [...kept, ...missing];
      }
      if (Array.isArray(parsed.hidden)) {
        navState.hidden = parsed.hidden.filter((id): id is string => typeof id === "string" && known.has(id));
      }
      if (Array.isArray(parsed.pinned)) {
        navState.pinned = parsed.pinned.filter((id): id is string => typeof id === "string" && known.has(id));
      }
    }
  } catch {
    /* defaults */
  }
}

export function toggleHidden(id: string): void {
  navState.hidden = navState.hidden.includes(id)
    ? navState.hidden.filter((entry) => entry !== id)
    : [...navState.hidden, id];
  persist();
}

export function togglePinned(id: string): void {
  navState.pinned = navState.pinned.includes(id)
    ? navState.pinned.filter((entry) => entry !== id)
    : [...navState.pinned, id];
  persist();
}

export function moveNav(id: string, direction: -1 | 1): void {
  const order = [...navState.order];
  const index = order.indexOf(id);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= order.length) return;
  [order[index], order[target]] = [order[target], order[index]];
  navState.order = order;
  persist();
}

export function resetNav(): void {
  navState.order = defaultNavOrder();
  navState.hidden = [];
  navState.pinned = [];
  persist();
}
