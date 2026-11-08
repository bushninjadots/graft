import { api, type AppInfo, type ToolInfo } from "../api";

type SessionState = {
  online: boolean;
  info: AppInfo | null;
  tools: ToolInfo[];
  toolsLoading: boolean;
  error: string | null;
};

export const session = $state<SessionState>({
  online: false,
  info: null,
  tools: [],
  toolsLoading: false,
  error: null,
});

export async function boot(): Promise<void> {
  try {
    session.info = await api.appInfo();
    session.online = true;
    session.error = null;
  } catch (error) {
    session.online = false;
    session.error = error instanceof Error ? error.message : String(error);
  }
}

export async function refreshTools(): Promise<void> {
  if (!session.online) return;
  session.toolsLoading = true;
  try {
    session.tools = await api.detectTools();
  } catch {
    session.tools = [];
  } finally {
    session.toolsLoading = false;
  }
}

export const foundTools = () => session.tools.filter((tool) => tool.found);
