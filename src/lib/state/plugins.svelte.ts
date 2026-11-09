import {
  api,
  type PluginRecord,
  type PluginRegistry,
  type RegistryPlugin,
  type TrustedKey,
} from "../api";

const REGISTRY_KEY = "plugins.registryUrl";

type PluginsState = {
  loaded: boolean;
  loading: boolean;
  error: string | null;
  installed: PluginRecord[];
  keys: TrustedKey[];
  registryUrl: string;
  registry: PluginRegistry | null;
  registryLoading: boolean;
  registryError: string | null;
  busyId: string | null;
  installError: string | null;
};

export const plugins = $state<PluginsState>({
  loaded: false,
  loading: false,
  error: null,
  installed: [],
  keys: [],
  registryUrl: "",
  registry: null,
  registryLoading: false,
  registryError: null,
  busyId: null,
  installError: null,
});

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function reloadLocal(): Promise<void> {
  const [installed, keys] = await Promise.all([
    api.pluginInstalled(),
    api.pluginKeys(),
  ]);
  plugins.installed = installed;
  plugins.keys = keys;
}

export async function loadPlugins(): Promise<void> {
  if (plugins.loading) return;
  plugins.loading = true;
  plugins.error = null;
  try {
    const [installed, keys, url] = await Promise.all([
      api.pluginInstalled(),
      api.pluginKeys(),
      api.getSetting(REGISTRY_KEY),
    ]);
    plugins.installed = installed;
    plugins.keys = keys;
    plugins.registryUrl = url ?? "";
    plugins.loaded = true;
  } catch (error) {
    plugins.error = message(error);
  } finally {
    plugins.loading = false;
  }
}

export async function refreshRegistry(): Promise<void> {
  const url = plugins.registryUrl.trim();
  plugins.registryError = null;
  plugins.registry = null;
  if (!url) return;
  plugins.registryLoading = true;
  try {
    plugins.registry = await api.pluginRegistry(url);
  } catch (error) {
    plugins.registryError = message(error);
  } finally {
    plugins.registryLoading = false;
  }
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

export function saveRegistryUrl(url: string): void {
  plugins.registryUrl = url.trim();
  plugins.registry = null;
  plugins.registryError = null;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api.setSetting(REGISTRY_KEY, plugins.registryUrl).catch(() => {});
  }, 400);
}

export async function install(
  entry: RegistryPlugin,
  trustKey: boolean,
): Promise<PluginRecord | null> {
  plugins.busyId = entry.id;
  plugins.installError = null;
  try {
    const record = await api.pluginInstall(entry, trustKey, true);
    await reloadLocal();
    return record;
  } catch (error) {
    plugins.installError = message(error);
    return null;
  } finally {
    plugins.busyId = null;
  }
}

export async function setEnabled(id: string, enabled: boolean): Promise<void> {
  plugins.busyId = id;
  plugins.installError = null;
  try {
    await api.pluginSetEnabled(id, enabled);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  } finally {
    plugins.busyId = null;
  }
}

export async function uninstall(id: string): Promise<void> {
  plugins.busyId = id;
  plugins.installError = null;
  try {
    await api.pluginUninstall(id, true);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  } finally {
    plugins.busyId = null;
  }
}

export async function removeKey(fingerprint: string): Promise<void> {
  plugins.installError = null;
  try {
    await api.pluginRemoveKey(fingerprint, true);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  }
}
