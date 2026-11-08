import { api } from "../api";

export type DashboardModuleId =
  | "account"
  | "projects"
  | "watchtower"
  | "environment"
  | "activity"
  | "shortcuts";

export type DashboardModuleMeta = {
  id: DashboardModuleId;
  title: string;
  hint: string;
  span: 1 | 2;
};

export const DASHBOARD_MODULES: DashboardModuleMeta[] = [
  { id: "account", title: "GitHub Account", hint: "Identity and scopes", span: 1 },
  { id: "projects", title: "My Projects", hint: "Local workspaces", span: 1 },
  { id: "watchtower", title: "Watchtower", hint: "Repository monitoring", span: 1 },
  { id: "environment", title: "Environment", hint: "Detected toolchain", span: 1 },
  { id: "activity", title: "Activity", hint: "Recent local events", span: 2 },
  { id: "shortcuts", title: "Shortcuts", hint: "Keyboard reference", span: 1 },
];

const DASHBOARD_KEY = "dashboard.layout";

type DashboardState = {
  order: DashboardModuleId[];
  hidden: DashboardModuleId[];
  editing: boolean;
};

export const dashboard = $state<DashboardState>({
  order: DASHBOARD_MODULES.map((module) => module.id),
  hidden: [],
  editing: false,
});

let saveTimer: ReturnType<typeof setTimeout> | undefined;

function persist(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api
      .setSetting(DASHBOARD_KEY, JSON.stringify({ order: dashboard.order, hidden: dashboard.hidden }))
      .catch(() => {});
  }, 200);
}

export async function loadDashboard(): Promise<void> {
  try {
    const raw = await api.getSetting(DASHBOARD_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<DashboardState>;
      const known = new Set(DASHBOARD_MODULES.map((module) => module.id));
      if (Array.isArray(parsed.order)) {
        const kept = parsed.order.filter(
          (id): id is DashboardModuleId => typeof id === "string" && known.has(id as DashboardModuleId),
        );
        const missing = DASHBOARD_MODULES.map((module) => module.id).filter((id) => !kept.includes(id));
        dashboard.order = [...kept, ...missing];
      }
      if (Array.isArray(parsed.hidden)) {
        dashboard.hidden = parsed.hidden.filter(
          (id): id is DashboardModuleId => typeof id === "string" && known.has(id as DashboardModuleId),
        );
      }
    }
  } catch {
    /* defaults */
  }
}

export function moduleMeta(id: DashboardModuleId): DashboardModuleMeta {
  return DASHBOARD_MODULES.find((module) => module.id === id) ?? DASHBOARD_MODULES[0];
}

export function toggleModule(id: DashboardModuleId): void {
  dashboard.hidden = dashboard.hidden.includes(id)
    ? dashboard.hidden.filter((entry) => entry !== id)
    : [...dashboard.hidden, id];
  persist();
}

export function moveModule(id: DashboardModuleId, direction: -1 | 1): void {
  const order = [...dashboard.order];
  const index = order.indexOf(id);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= order.length) return;
  [order[index], order[target]] = [order[target], order[index]];
  dashboard.order = order;
  persist();
}

export function dropModule(dragged: DashboardModuleId, target: DashboardModuleId): void {
  if (dragged === target) return;
  const order = dashboard.order.filter((id) => id !== dragged);
  const index = order.indexOf(target);
  if (index < 0) return;
  order.splice(index, 0, dragged);
  dashboard.order = order;
  persist();
}

export function resetDashboard(): void {
  dashboard.order = DASHBOARD_MODULES.map((module) => module.id);
  dashboard.hidden = [];
  persist();
}
