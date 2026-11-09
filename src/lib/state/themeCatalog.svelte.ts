import { DEFAULT_THEME_ID, themes, type Theme } from "../themes";
import type { PluginRecord } from "../api";
import { plugins } from "./plugins.svelte";

export function pluginTheme(record: PluginRecord): Theme | null {
  if (record.kind !== "theme" || !record.theme) return null;
  return {
    id: `plugin:${record.id}`,
    name: record.name,
    blurb: record.theme.blurb,
    tokens: { scheme: record.theme.scheme, ...record.theme.tokens },
  };
}

export function allThemes(): Theme[] {
  const extra = plugins.installed
    .filter((plugin) => plugin.kind === "theme" && plugin.enabled && plugin.verified)
    .map(pluginTheme)
    .filter((theme): theme is Theme => !!theme);
  return [...themes, ...extra];
}

export function findTheme(id: string): Theme {
  return (
    allThemes().find((theme) => theme.id === id) ??
    themes.find((theme) => theme.id === DEFAULT_THEME_ID) ??
    themes[0]
  );
}

export function reconcileThemeId(id: string): string {
  return allThemes().some((theme) => theme.id === id) ? id : DEFAULT_THEME_ID;
}
