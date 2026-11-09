import { api } from "../api";
import { DEFAULT_THEME_ID, type Theme } from "../themes";
import { allThemes, findTheme, reconcileThemeId } from "./themeCatalog.svelte";

export type Density = "compact" | "comfortable" | "roomy";

type AppearanceState = {
  themeId: string;
  density: Density;
  radius: number;
  sidebarCollapsed: boolean;
  loaded: boolean;
};

const APPEARANCE_KEY = "appearance";

export const appearance = $state<AppearanceState>({
  themeId: DEFAULT_THEME_ID,
  density: "compact",
  radius: 3,
  sidebarCollapsed: false,
  loaded: false,
});

export function currentTheme(): Theme {
  return findTheme(appearance.themeId);
}

/** Writes the active theme tokens onto :root. The only place colours are set. */
export function applyAppearance(): void {
  if (typeof document === "undefined") return;
  const theme = currentTheme();
  const root = document.documentElement;
  const t = theme.tokens;

  const tokens: Record<string, string> = {
    "--bg": t.bg,
    "--bg-elev": t.bgElev,
    "--surface": t.surface,
    "--surface-alt": t.surfaceAlt,
    "--border": t.border,
    "--border-strong": t.borderStrong,
    "--text": t.text,
    "--text-muted": t.textMuted,
    "--text-faint": t.textFaint,
    "--accent": t.accent,
    "--accent-soft": t.accentSoft,
    "--accent-ink": t.accentInk,
    "--ok": t.ok,
    "--warn": t.warn,
    "--danger": t.danger,
    "--info": t.info,
    "--shadow": t.shadow,
    "--radius": `${appearance.radius}px`,
  };

  for (const [name, value] of Object.entries(tokens)) {
    root.style.setProperty(name, value);
  }
  root.style.colorScheme = t.scheme;
  root.dataset.theme = theme.id;
  root.dataset.density = appearance.density;
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

function persist(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api
      .setSetting(
        APPEARANCE_KEY,
        JSON.stringify({
          themeId: appearance.themeId,
          density: appearance.density,
          radius: appearance.radius,
          sidebarCollapsed: appearance.sidebarCollapsed,
        }),
      )
      .catch(() => {
        /* offline: preferences stay in memory for this session */
      });
  }, 200);
}

export async function loadAppearance(): Promise<void> {
  try {
    const raw = await api.getSetting(APPEARANCE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<AppearanceState>;
      if (
        typeof parsed.themeId === "string" &&
        allThemes().some((theme) => theme.id === parsed.themeId)
      ) {
        appearance.themeId = parsed.themeId;
      }
      if (
        parsed.density === "compact" ||
        parsed.density === "comfortable" ||
        parsed.density === "roomy"
      ) {
        appearance.density = parsed.density;
      }
      if (typeof parsed.radius === "number" && parsed.radius >= 0 && parsed.radius <= 12) {
        appearance.radius = parsed.radius;
      }
      if (typeof parsed.sidebarCollapsed === "boolean") {
        appearance.sidebarCollapsed = parsed.sidebarCollapsed;
      }
    }
  } catch {
    /* keep defaults */
  }
  appearance.themeId = reconcileThemeId(appearance.themeId);
  appearance.loaded = true;
  applyAppearance();
}

export function setTheme(id: string): void {
  if (!allThemes().some((theme) => theme.id === id)) return;
  appearance.themeId = id;
  applyAppearance();
  persist();
}

export function reconcileTheme(): void {
  const next = reconcileThemeId(appearance.themeId);
  if (next === appearance.themeId) return;
  appearance.themeId = next;
  applyAppearance();
  persist();
}

export function setDensity(density: Density): void {
  appearance.density = density;
  applyAppearance();
  persist();
}

export function setRadius(radius: number): void {
  appearance.radius = Math.min(12, Math.max(0, radius));
  applyAppearance();
  persist();
}

export function toggleSidebar(): void {
  appearance.sidebarCollapsed = !appearance.sidebarCollapsed;
  persist();
}

export function nextTheme(direction: 1 | -1 = 1): void {
  const order = allThemes().map((theme) => theme.id);
  const index = order.indexOf(appearance.themeId);
  const next = order[(index + direction + order.length) % order.length];
  setTheme(next);
}
