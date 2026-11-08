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

export type GithubRepoDetail = {
  id: number;
  name: string;
  fullName: string;
  private: boolean;
  fork: boolean;
  archived: boolean;
  description: string | null;
  language: string | null;
  stargazersCount: number;
  watchersCount: number;
  forksCount: number;
  openIssuesCount: number;
  size: number;
  topics: string[];
  defaultBranch: string;
  htmlUrl: string;
  cloneUrl: string;
  sshUrl: string;
  pushedAt: string | null;
  updatedAt: string;
  createdAt: string | null;
  license: { name: string; spdxId: string | null } | null;
  owner: { login: string; avatarUrl: string } | null;
  starred: boolean;
};

export type RepoEntry = {
  name: string;
  path: string;
  type: "file" | "dir" | "symlink" | "submodule";
  size: number;
  sha: string;
  htmlUrl: string | null;
  downloadUrl: string | null;
};

export type RepoFile = {
  name: string;
  path: string;
  size: number;
  sha: string;
  htmlUrl: string;
  content: string | null;
  truncated: boolean;
  binary: boolean;
};

export type RepoBranch = { name: string; sha: string; protected: boolean };

export type RepoCommit = {
  sha: string;
  short: string;
  message: string;
  authorName: string;
  authorLogin: string | null;
  authorAvatar: string | null;
  date: string | null;
  htmlUrl: string;
};

export type RepoCommitFile = {
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  changes: number;
  patch: string | null;
};

export type RepoCommitDetail = {
  sha: string;
  message: string;
  authorName: string;
  authorLogin: string | null;
  authorAvatar: string | null;
  date: string | null;
  additions: number;
  deletions: number;
  htmlUrl: string;
  files: RepoCommitFile[];
};

export type RepoPull = {
  number: number;
  title: string;
  state: string;
  draft: boolean;
  merged: boolean;
  authorLogin: string;
  authorAvatar: string;
  body: string;
  createdAt: string;
  updatedAt: string;
  comments: number;
  additions: number;
  deletions: number;
  changedFiles: number;
  headRef: string;
  baseRef: string;
  htmlUrl: string;
};

export type RepoPullFile = {
  filename: string;
  status: string;
  additions: number;
  deletions: number;
  patch: string | null;
};

export type RepoPullDetail = { pull: RepoPull; files: RepoPullFile[] };

export type ForkResult = { fullName: string; htmlUrl: string; cloneUrl: string };

export type GitActionResult = {
  action: string;
  ok: boolean;
  output: string;
  error: string | null;
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

  repoDetail: (owner: string, repo: string) =>
    call<GithubRepoDetail>("github_repo_detail", { owner, repo }),
  repoReadme: (owner: string, repo: string) =>
    call<RepoFile | null>("github_repo_readme", { owner, repo }),
  repoContents: (owner: string, repo: string, path: string, gitRef: string | null) =>
    call<RepoEntry[]>("github_repo_contents", { owner, repo, path, gitRef }),
  repoFile: (owner: string, repo: string, path: string, gitRef: string | null) =>
    call<RepoFile>("github_repo_file", { owner, repo, path, gitRef }),
  repoBranches: (owner: string, repo: string) =>
    call<RepoBranch[]>("github_repo_branches", { owner, repo }),
  repoCommits: (owner: string, repo: string, gitRef: string | null, page: number) =>
    call<RepoCommit[]>("github_repo_commits", { owner, repo, gitRef, page }),
  repoCommit: (owner: string, repo: string, sha: string) =>
    call<RepoCommitDetail>("github_repo_commit", { owner, repo, sha }),
  repoPulls: (owner: string, repo: string, state: string) =>
    call<RepoPull[]>("github_repo_pulls", { owner, repo, state }),
  repoPull: (owner: string, repo: string, number: number) =>
    call<RepoPullDetail>("github_repo_pull", { owner, repo, number }),
  forkRepo: (owner: string, repo: string) =>
    call<ForkResult>("github_fork", { owner, repo }),
  setStar: (owner: string, repo: string, starred: boolean) =>
    call<boolean>("github_set_star", { owner, repo, starred }),
  cloneDestination: (owner: string, repo: string) =>
    call<string>("clone_destination", { owner, repo }),
  cloneRepo: (url: string, dest: string) => call<string>("clone_repo", { url, dest }),
  gitAction: (path: string, action: string) =>
    call<GitActionResult>("git_action", { path, action }),
};
