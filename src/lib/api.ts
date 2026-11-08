import { invoke } from "@tauri-apps/api/core";

export type Project = {
  id: number;
  name: string;
  path: string;
  description: string;
  createdAt: number;
  updatedAt: number;
};

export type ToolInfo = {
  id: string;
  name: string;
  found: boolean;
  version: string | null;
  path: string | null;
};

export type AppInfo = {
  version: string;
  dataDir: string;
  dbPath: string;
  schemaVersion: number;
};

export type GithubUser = {
  login: string;
  name: string | null;
  avatarUrl: string;
  htmlUrl: string;
  publicRepos: number;
  totalPrivateRepos: number | null;
};

export type GithubRepo = {
  id: number;
  name: string;
  fullName: string;
  private: boolean;
  fork: boolean;
  archived: boolean;
  description: string | null;
  language: string | null;
  stargazersCount: number;
  forksCount: number;
  updatedAt: string;
  pushedAt: string | null;
  defaultBranch: string;
  htmlUrl: string;
  owner: { login: string } | null;
};

export type GithubAuthStatus = {
  clientId: string | null;
  signedIn: boolean;
  user: GithubUser | null;
  cachedUser: GithubUser | null;
  tokenError: string | null;
  keyringError: string | null;
  scopes: string | null;
};

export type DeviceStart = {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
};

export type DevicePoll =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "complete"; user: GithubUser; scopes: string | null }
  | { status: "failed"; message: string };

export type RepoPage = {
  page: number;
  repos: GithubRepo[];
  hasMore: boolean;
};

export type GitStatus = {
  path: string;
  isRepo: boolean;
  toplevel: string | null;
  branch: string | null;
  detached: boolean;
  ahead: number;
  behind: number;
  changed: number;
  remote: string | null;
  github: string | null;
  error: string | null;
};

/** True when running inside the Tauri webview rather than a plain browser tab. */
export const isDesktop =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const OFFLINE_MESSAGE =
  "The GRAFT backend is not reachable. Start the desktop app with `npm run app:dev`.";

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop) throw new Error(OFFLINE_MESSAGE);
  return invoke<T>(command, args);
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  getSetting: (key: string) => call<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => call<void>("set_setting", { key, value }),
  listProjects: () => call<Project[]>("list_projects"),
  createProject: (name: string, path: string, description: string) =>
    call<Project>("create_project", { name, path, description }),
  deleteProject: (id: number) => call<void>("delete_project", { id }),
  detectTools: () => call<ToolInfo[]>("detect_tools"),
  gitStatus: (path: string) => call<GitStatus>("project_git_status", { path }),
  openExternal: (url: string) => call<void>("open_external", { url }),

  githubStatus: () => call<GithubAuthStatus>("github_status"),
  setGithubClientId: (clientId: string) =>
    call<void>("github_set_client_id", { clientId }),
  startDeviceFlow: () => call<DeviceStart>("github_start_device"),
  pollDeviceFlow: (deviceCode: string) =>
    call<DevicePoll>("github_poll_device", { deviceCode }),
  saveGithubToken: (token: string) => call<GithubUser>("github_save_token", { token }),
  signOutGithub: () => call<boolean>("github_sign_out"),
  listRepos: (page: number) => call<RepoPage>("github_list_repos", { page }),
};
