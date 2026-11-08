import { Channel, invoke } from "@tauri-apps/api/core";
import type { ProfileDesign } from "./profile";

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
  topics: string[];
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

export type RepoSearchPage = {
  page: number;
  totalCount: number;
  repos: GithubRepo[];
  hasMore: boolean;
};

export type GithubNotification = {
  id: string;
  reason: string;
  unread: boolean;
  updatedAt: string;
  title: string;
  subjectType: string;
  number: number | null;
  repo: GithubRepo | null;
};

export type RepoPermissions = {
  admin: boolean;
  maintain: boolean;
  push: boolean;
  triage: boolean;
  pull: boolean;
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
  permissions: RepoPermissions | null;
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
  parents: string[];
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
  headSha: string;
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

export type RepoPullReview = {
  id: number;
  state: string;
  body: string;
  authorLogin: string;
  authorAvatar: string;
  submittedAt: string | null;
  htmlUrl: string | null;
};

export type RepoPullComment = {
  id: number;
  body: string;
  path: string;
  line: number | null;
  originalLine: number | null;
  side: string | null;
  authorLogin: string;
  authorAvatar: string;
  createdAt: string;
  htmlUrl: string;
};

export type WatchtowerRepo = {
  id: number;
  owner: string;
  name: string;
  fullName: string;
  htmlUrl: string;
  description: string | null;
  language: string | null;
  private: boolean;
  lastPolledAt: number | null;
  lastError: string | null;
  createdAt: number;
};

export type WatchtowerEvent = {
  id: number;
  repoId: number;
  repoFullName: string;
  category: string;
  kind: string;
  action: string;
  title: string;
  detail: string;
  actor: string;
  url: string;
  occurredAt: string;
  seen: boolean;
  createdAt: number;
};

export type WatchtowerSnapshot = {
  repos: WatchtowerRepo[];
  events: WatchtowerEvent[];
  unread: number;
};

export type WatchtowerPoll = {
  checked: number;
  notModified: number;
  newEvents: WatchtowerEvent[];
  errors: string[];
};

export type VisitedRepo = {
  fullName: string;
  owner: string;
  name: string;
  htmlUrl: string;
  description: string | null;
  language: string | null;
  topics: string[];
  stars: number;
  forks: number;
  defaultBranch: string;
  private: boolean;
  visitCount: number;
  lastVisitedAt: number;
};

export type WatchtowerRecommendation = {
  repo: GithubRepo;
  reason: string;
  score: number;
};

export type WatchtowerDiscovery = {
  starred: GithubRepo[];
  visited: VisitedRepo[];
  recommended: WatchtowerRecommendation[];
  errors: string[];
};

export type TerminalEvent =
  | { type: "output"; data: string }
  | { type: "exit" }
  | { type: "error"; message: string };

export type TerminalSessionInfo = {
  id: string;
  shell: string;
  cwd: string;
};

export type TerminalEnvironment = {
  cwd: string;
  shells: string[];
  defaultShell: string;
};

export type SavedSearch = {
  id: number;
  name: string;
  query: string;
  language: string;
  topic: string;
  author: string;
  sort: string;
  createdAt: number;
};

export type Collection = {
  id: number;
  name: string;
  description: string;
  repoCount: number;
  createdAt: number;
  updatedAt: number;
};

export type CollectionRepo = {
  id: number;
  collectionId: number;
  owner: string;
  name: string;
  fullName: string;
  htmlUrl: string;
  description: string | null;
  language: string | null;
  private: boolean;
  topics: string[];
  stars: number;
  forks: number;
  defaultBranch: string;
  status: string;
  notes: string;
  createdAt: number;
  updatedAt: number;
};

export type CollectionDetail = {
  collection: Collection;
  repos: CollectionRepo[];
};

export type ForkResult = { fullName: string; htmlUrl: string; cloneUrl: string };

export type GitActionResult = {
  action: string;
  ok: boolean;
  output: string;
  error: string | null;
};

export type GitBranch = {
  name: string;
  current: boolean;
  upstream: string | null;
};

export type ProfileReadme = {
  exists: boolean;
  content: string | null;
  sha: string | null;
  htmlUrl: string | null;
};

export type PublishResult = {
  sha: string;
  commitSha: string;
  htmlUrl: string | null;
};

export type RepoIssue = {
  number: number;
  title: string;
  state: string;
  body: string;
  authorLogin: string;
  authorAvatar: string;
  labels: string[];
  comments: number;
  createdAt: string;
  updatedAt: string;
  htmlUrl: string;
};

export type ProfileRepoResult = { fullName: string; htmlUrl: string };

export type FileWriteResult = {
  sha: string;
  commitSha: string;
  htmlUrl: string | null;
  path: string;
};

/** Live data behind Profile Studio. Every field comes from the GitHub API. */
export type ProfileSnapshot = {
  login: string;
  name: string | null;
  avatarUrl: string;
  bio: string | null;
  company: string | null;
  location: string | null;
  websiteUrl: string | null;
  twitterUsername: string | null;
  status: { emoji: string; message: string } | null;
  socials: { provider: string; url: string }[];
  followers: number;
  following: number;
  publicRepos: number;
  gists: number;
  stars: number;
  createdAt: string;
  pinned: ProfileRepo[];
  topRepos: ProfileRepo[];
  contributionDays: { date: string; count: number; weekday: number }[];
  totalContributions: number;
  commits: number;
  pullRequests: number;
  issues: number;
  reviews: number;
  privateContributions: number;
  organizations: { login: string; avatarUrl: string; description: string | null }[];
  fetchedAt: number;
};

export type ProfileRepo = {
  nameWithOwner: string;
  name: string;
  description: string | null;
  stars: number;
  forks: number;
  url: string;
  isFork: boolean;
  language: string | null;
  languageColor: string | null;
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
  searchRepos: (
    query: string,
    language: string,
    topic: string,
    author: string,
    sort: string,
    page: number,
  ) =>
    call<RepoSearchPage>("github_search_repos", {
      query,
      language,
      topic,
      author,
      sort,
      page,
    }),
  githubNotifications: () =>
    call<GithubNotification[]>("github_notifications"),
  watchtowerSnapshot: () => call<WatchtowerSnapshot>("watchtower_snapshot"),
  watchRepo: (owner: string, repo: string) =>
    call<WatchtowerRepo>("watchtower_watch", { owner, repo }),
  unwatchRepo: (id: number) => call<void>("watchtower_unwatch", { id }),
  pollWatchtower: (categories: string[]) =>
    call<WatchtowerPoll>("watchtower_poll", { categories }),
  markWatchtowerRead: (id: number | null) =>
    call<number>("watchtower_mark_read", { id }),
  watchtowerDiscover: () =>
    call<WatchtowerDiscovery>("watchtower_discover"),
  recordRepoVisit: (repo: GithubRepo) =>
    call<void>("watchtower_record_visit", {
      repo: {
        owner: repo.owner?.login ?? repo.fullName.split("/")[0] ?? "",
        name: repo.name,
        htmlUrl: repo.htmlUrl,
        description: repo.description,
        language: repo.language,
        topics: repo.topics,
        stars: repo.stargazersCount,
        forks: repo.forksCount,
        defaultBranch: repo.defaultBranch,
        private: repo.private,
      },
    }),

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
  repoPullReviews: (owner: string, repo: string, number: number) =>
    call<RepoPullReview[]>("github_repo_pull_reviews", { owner, repo, number }),
  repoPullComments: (owner: string, repo: string, number: number) =>
    call<RepoPullComment[]>("github_repo_pull_comments", { owner, repo, number }),
  submitPullReview: (
    owner: string,
    repo: string,
    number: number,
    event: string,
    body: string,
  ) =>
    call<RepoPullReview>("github_submit_pull_review", { owner, repo, number, event, body }),
  createPullComment: (
    owner: string,
    repo: string,
    number: number,
    body: string,
    commitId: string,
    path: string,
    line: number,
    side: string,
  ) =>
    call<RepoPullComment>("github_create_pull_comment", {
      owner,
      repo,
      number,
      body,
      commitId,
      path,
      line,
      side,
    }),
  forkRepo: (owner: string, repo: string) =>
    call<ForkResult>("github_fork", { owner, repo }),
  setStar: (owner: string, repo: string, starred: boolean) =>
    call<boolean>("github_set_star", { owner, repo, starred }),
  cloneDestination: (owner: string, repo: string) =>
    call<string>("clone_destination", { owner, repo }),
  cloneRepo: (url: string, dest: string) => call<string>("clone_repo", { url, dest }),
  gitAction: (path: string, action: string) =>
    call<GitActionResult>("git_action", { path, action }),
  gitBranches: (path: string) => call<GitBranch[]>("git_branches", { path }),
  gitStage: (path: string) => call<GitActionResult>("git_stage", { path }),
  gitCommit: (path: string, message: string, stageAll: boolean) =>
    call<GitActionResult>("git_commit", { path, message, stageAll }),
  gitCreateBranch: (path: string, name: string, switchTo: boolean) =>
    call<GitActionResult>("git_create_branch", { path, name, switch: switchTo }),
  gitSwitchBranch: (path: string, name: string) =>
    call<GitActionResult>("git_switch_branch", { path, name }),

  terminalEnvironment: () => call<TerminalEnvironment>("terminal_environment"),
  terminalStart: (
    cwd: string,
    shell: string,
    cols: number,
    rows: number,
    onEvent: (event: TerminalEvent) => void,
  ) => {
    const channel = new Channel<TerminalEvent>();
    channel.onmessage = onEvent;
    return call<TerminalSessionInfo>("terminal_start", {
      cwd,
      shell,
      cols,
      rows,
      channel,
    });
  },
  terminalWrite: (sessionId: string, data: string) =>
    call<void>("terminal_write", { sessionId, data }),
  terminalResize: (sessionId: string, cols: number, rows: number) =>
    call<void>("terminal_resize", { sessionId, cols, rows }),
  terminalClose: (sessionId: string) =>
    call<void>("terminal_close", { sessionId }),

  savedSearches: () => call<SavedSearch[]>("explore_saved_searches"),
  saveSearch: (
    name: string,
    query: string,
    language: string,
    topic: string,
    author: string,
    sort: string,
  ) =>
    call<SavedSearch>("explore_save_search", {
      name,
      query,
      language,
      topic,
      author,
      sort,
    }),
  deleteSavedSearch: (id: number) =>
    call<void>("explore_delete_search", { id }),
  collections: () => call<Collection[]>("collections_list"),
  createCollection: (name: string, description: string) =>
    call<Collection>("collections_create", { name, description }),
  deleteCollection: (id: number) =>
    call<void>("collections_delete", { id }),
  collectionDetail: (id: number) =>
    call<CollectionDetail>("collection_detail", { id }),
  addCollectionRepo: (collectionId: number, owner: string, repo: string) =>
    call<CollectionRepo>("collection_add_repo", { collectionId, owner, repo }),
  updateCollectionRepo: (id: number, status: string, notes: string) =>
    call<CollectionRepo>("collection_update_repo", { id, status, notes }),
  removeCollectionRepo: (id: number) =>
    call<void>("collection_remove_repo", { id }),

  githubProfileReadme: (login: string) =>
    call<ProfileReadme>("github_profile_readme", { login }),
  githubPublishProfile: (
    login: string,
    markdown: string,
    sha: string | null,
    message: string,
  ) => call<PublishResult>("github_publish_profile", { login, markdown, sha, message }),

  githubProfileSnapshot: () => call<ProfileSnapshot>("github_profile_snapshot"),
  setProfileStatus: (emoji: string | null, message: string) =>
    call<ProfileSnapshot["status"]>("github_set_profile_status", { emoji, message }),
  profileRender: (design: ProfileDesign, snapshot: ProfileSnapshot) =>
    call<string>("profile_render", { design, snapshot }),

  repoIssues: (owner: string, repo: string, state: string) =>
    call<RepoIssue[]>("github_repo_issues", { owner, repo, state }),
  createIssue: (owner: string, repo: string, title: string, body: string) =>
    call<RepoIssue>("github_create_issue", { owner, repo, title, body }),
  createProfileRepo: (login: string) =>
    call<ProfileRepoResult>("github_create_profile_repo", { login }),

  repoSaveFile: (
    owner: string,
    repo: string,
    path: string,
    content: string,
    sha: string | null,
    branch: string | null,
    message: string,
  ) =>
    call<FileWriteResult>("github_save_file", {
      owner,
      repo,
      path,
      content,
      sha,
      branch,
      message,
    }),
};
