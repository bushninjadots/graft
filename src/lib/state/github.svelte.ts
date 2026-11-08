import {
  api,
  type GithubAuthStatus,
  type GithubRepo,
  type GithubUser,
} from "../api";
import { navigate, notify } from "./ui.svelte";

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

/** Opens the repository detail surface for `repo`. */
export function openRepo(repo: GithubRepo): void {
  github.selected = repo;
  navigate("repository");
}

export function closeRepo(): void {
  navigate("repositories");
}

export async function loadGithub(): Promise<void> {
  github.loading = true;
  try {
    github.status = await api.githubStatus();
    github.error = null;
  } catch (cause) {
    github.error = message(cause);
  } finally {
    github.loading = false;
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
      repo.private ? "private" : "public"
    }`.toLowerCase();
    return parts.every((part) => haystack.includes(part));
  });
}
