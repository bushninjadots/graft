import {
  api,
  type GithubAuthStatus,
  type GithubRepo,
  type GithubUser,
  type RateLimits,
} from "../api";
import { navigate, notify, ui } from "./ui.svelte";

export type RepoScope = "mine" | "explore";
export type RepoSort = "best" | "stars" | "forks" | "updated";

type GithubState = {
  loading: boolean;
  status: GithubAuthStatus | null;
  error: string | null;
  repos: GithubRepo[];
  reposLoading: boolean;
  reposError: string | null;
  page: number;
  hasMore: boolean;
  filter: string;
  selected: GithubRepo | null;
  scope: RepoScope;
  syncing: boolean;
  syncedAt: number | null;
  rate: RateLimits | null;
  rateAt: number;
  rateLoading: boolean;
  discover: {
    query: string;
    language: string;
    topic: string;
    author: string;
    sort: RepoSort;
    results: GithubRepo[];
    totalCount: number;
    page: number;
    hasMore: boolean;
    loading: boolean;
    error: string | null;
    searched: boolean;
  };
};

export const github = $state<GithubState>({
  loading: false,
  status: null,
  error: null,
  repos: [],
  reposLoading: false,
  reposError: null,
  page: 1,
  hasMore: false,
  filter: "",
  selected: null,
  scope: "mine",
  syncing: false,
  syncedAt: null,
  rate: null,
  rateAt: 0,
  rateLoading: false,
  discover: {
    query: "",
    language: "",
    topic: "",
    author: "",
    sort: "best",
    results: [],
    totalCount: 0,
    page: 1,
    hasMore: false,
    loading: false,
    error: null,
    searched: false,
  },
});

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** The verified profile when online, otherwise the last profile seen. */
export function githubUser(): GithubUser | null {
  return github.status?.user ?? github.status?.cachedUser ?? null;
}

export function isSignedIn(): boolean {
  return github.status?.signedIn ?? false;
}

/** Opens the repository detail surface for `repo` and records a local visit. */
export function openRepo(repo: GithubRepo): void {
  github.selected = repo;
  void api.recordRepoVisit(repo).catch(() => undefined);
  navigate("repository");
}

export function closeRepo(): void {
  navigate("repositories");
}

/** Opens a repository by coordinate after reading its live metadata. */
export async function openRepoByCoords(owner: string, repo: string): Promise<void> {
  try {
    const detail = await api.repoDetail(owner, repo);
    const selected: GithubRepo = {
      id: detail.id,
      name: detail.name,
      fullName: detail.fullName,
      private: detail.private,
      fork: detail.fork,
      archived: detail.archived,
      description: detail.description,
      language: detail.language,
      topics: detail.topics ?? [],
      stargazersCount: detail.stargazersCount,
      forksCount: detail.forksCount,
      updatedAt: detail.updatedAt,
      pushedAt: detail.pushedAt,
      defaultBranch: detail.defaultBranch,
      htmlUrl: detail.htmlUrl,
      owner: { login: detail.owner?.login ?? owner },
    };
    github.selected = selected;
    void api.recordRepoVisit(selected).catch(() => undefined);
    navigate("repository");
  } catch (cause) {
    notify(cause instanceof Error ? cause.message : String(cause), "danger");
  }
}

export async function loadGithub(): Promise<void> {
  github.loading = true;
  try {
    github.status = await api.githubStatus();
    github.error = null;
    if (github.status.signedIn) void loadRateLimit();
    else github.rate = null;
  } catch (cause) {
    github.error = message(cause);
  } finally {
    github.loading = false;
  }
}

/** Reads the live API quota; the endpoint itself does not consume quota. */
export async function loadRateLimit(force = false): Promise<void> {
  if (!isSignedIn() || github.rateLoading) return;
  if (!force && github.rateAt && Date.now() - github.rateAt < 60_000) return;
  github.rateLoading = true;
  try {
    github.rate = await api.githubRateLimit();
    github.rateAt = Date.now();
  } catch {
    // Quota is diagnostic; failures stay out of the notification stream.
  } finally {
    github.rateLoading = false;
  }
}

export async function setClientId(clientId: string): Promise<boolean> {
  try {
    await api.setGithubClientId(clientId);
    await loadGithub();
    return true;
  } catch (cause) {
    notify(message(cause), "danger");
    return false;
  }
}

export async function signOut(): Promise<void> {
  try {
    await api.signOutGithub();
    github.repos = [];
    github.page = 1;
    github.hasMore = false;
    github.selected = null;
    github.syncedAt = null;
    github.rate = null;
    github.rateAt = 0;
    github.discover.results = [];
    github.discover.totalCount = 0;
    github.discover.searched = false;
    await loadGithub();
    notify("Signed out of GitHub", "ok");
  } catch (cause) {
    notify(message(cause), "danger");
  }
}

export async function loadRepos(reset = false): Promise<void> {
  if (github.reposLoading) return;
  github.reposLoading = true;
  github.reposError = null;
  try {
    const page = reset ? 1 : github.page;
    const result = await api.listRepos(page);
    github.repos = reset ? result.repos : [...github.repos, ...result.repos];
    github.page = result.page + 1;
    github.hasMore = result.hasMore;
  } catch (cause) {
    github.reposError = message(cause);
  } finally {
    github.reposLoading = false;
  }
}

export function visibleRepos(): GithubRepo[] {
  const query = github.filter.trim().toLowerCase();
  if (!query) return github.repos;
  const parts = query.split(/\s+/);
  return github.repos.filter((repo) => {
    const haystack = `${repo.fullName} ${repo.description ?? ""} ${repo.language ?? ""} ${
      (repo.topics ?? []).join(" ")
    } ${repo.private ? "private" : "public"}`.toLowerCase();
    return parts.every((part) => haystack.includes(part));
  });
}

export function setRepoScope(scope: RepoScope): void {
  github.scope = scope;
  void api.setSetting("repos.scope", scope).catch(() => undefined);
}

/** Searches public GitHub repositories through the real search API. */
export async function searchRepos(reset = true): Promise<void> {
  if (github.discover.loading) return;
  const discover = github.discover;
  if (
    !discover.query.trim() &&
    !discover.language.trim() &&
    !discover.topic.trim() &&
    !discover.author.trim()
  ) {
    discover.error = "Enter a search term, language, topic or author.";
    return;
  }
  discover.loading = true;
  discover.error = null;
  try {
    const page = reset ? 1 : discover.page;
    const result = await api.searchRepos(
      discover.query,
      discover.language,
      discover.topic,
      discover.author,
      discover.sort,
      page,
    );
    discover.results = reset ? result.repos : [...discover.results, ...result.repos];
    discover.totalCount = result.totalCount;
    discover.page = result.page + 1;
    discover.hasMore = result.hasMore;
    discover.searched = true;
  } catch (cause) {
    discover.error = message(cause);
  } finally {
    discover.loading = false;
  }
}

/** Refreshes GitHub-backed state. Shared by focus, the interval and the sync button. */
export async function syncGithub(force = false): Promise<void> {
  if (github.syncing) return;
  if (!force && github.syncedAt && Date.now() - github.syncedAt < 20_000) return;
  github.syncing = true;
  try {
    await loadGithub();
    if (!isSignedIn()) return;
    const activeRepoView =
      ui.view === "repositories" || ui.view === "repository" || ui.view === "home";
    if (github.repos.length > 0 && (force || activeRepoView)) await loadRepos(true);
    if (github.discover.searched && (force || github.scope === "explore")) {
      await searchRepos(true);
    }
    github.syncedAt = Date.now();
  } finally {
    github.syncing = false;
  }
}
