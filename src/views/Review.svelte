<script lang="ts">
  import { onMount } from "svelte";
  import type { GithubRepo } from "../lib/api";
  import { plural, relativeTime } from "../lib/format";
  import {
    github,
    isSignedIn,
    loadGithub,
    loadRepos,
  } from "../lib/state/github.svelte";
  import { navigate, ui } from "../lib/state/ui.svelte";
  import Icon from "../lib/components/Icon.svelte";

  let filter = $state("");

  const signedIn = $derived(isSignedIn());
  const repos = $derived.by(() => {
    const query = filter.trim().toLowerCase();
    if (!query) return github.repos;
    return github.repos.filter((repo) =>
      `${repo.fullName} ${repo.description ?? ""} ${repo.language ?? ""}`
        .toLowerCase()
        .includes(query),
    );
  });

  onMount(() => {
    if (!github.status) void loadGithub();
    if (isSignedIn()) void loadRepos();
  });

  function review(repo: GithubRepo): void {
    github.selected = repo;
    ui.repositoryFocus = { tab: "pulls", path: null };
    navigate("repository");
  }
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">Code Review</h1>
      <p class="view-sub">
        Choose a repository to open its pull-request workspace: descriptions, unified or
        side-by-side diffs, inline comments, approvals and change requests.
      </p>
    </div>
    <div class="head-actions">
      <button class="btn ghost" onclick={() => navigate("repositories")}>All repositories</button>
      <button class="btn" onclick={() => void loadRepos(true)} disabled={github.reposLoading || !signedIn}>
        <span class:spin={github.reposLoading}><Icon name="refresh" size={12} /></span>
        {github.reposLoading ? "Refreshing…" : "Refresh"}
      </button>
    </div>
  </div>

  {#if !signedIn}
    <div class="panel empty">
      <Icon name="review" size={24} />
      <p class="empty-title">Connect GitHub to review pull requests</p>
      <p class="empty-copy">
        Reviews use your own GitHub identity. GRAFT asks for confirmation before it writes a
        review, approval or inline comment.
      </p>
      <button class="btn primary" onclick={() => navigate("repositories")}>Connect GitHub</button>
    </div>
  {:else}
    <div class="panel search-panel">
      <Icon name="search" size={13} />
      <input class="search" bind:value={filter} placeholder="Filter your repositories…" spellcheck="false" aria-label="Filter repositories" />
      <span class="mono faint">{plural(repos.length, "repository")}</span>
    </div>

    {#if github.reposLoading && github.repos.length === 0}
      <p class="state mono">reading your repositories from api.github.com…</p>
    {:else if github.reposError}
      <div class="panel notice">
        <p class="tone-danger">{github.reposError}</p>
        <button class="btn" onclick={() => void loadRepos(true)}>Retry</button>
      </div>
    {:else if repos.length === 0}
      <p class="state">No repository matches that filter.</p>
    {:else}
      <div class="review-grid">
        {#each repos as repo (repo.id)}
          <button class="panel repo-card" onclick={() => review(repo)}>
            <span class="repo-top">
              <span class="repo-icon"><Icon name="folder-git" size={14} /></span>
              <span class="repo-name">{repo.fullName}</span>
              {#if repo.private}<span class="badge">private</span>{/if}
              <span class="open"><Icon name="chevron-right" size={13} /></span>
            </span>
            {#if repo.description}<span class="repo-desc">{repo.description}</span>{/if}
            <span class="facts mono">
              <span>{repo.language ?? "no language"}</span>
              <span>★ {repo.stargazersCount}</span>
              <span>updated {relativeTime(Date.parse(repo.pushedAt ?? repo.updatedAt) / 1000)}</span>
            </span>
            <span class="review-cta mono">open pull requests →</span>
          </button>
        {/each}
      </div>
    {/if}

    <div class="panel note">
      <span class="mono-label">how review works</span>
      <p>
        Open a repository, choose Pull requests, then a pull request. GRAFT loads its files and
        conversations from GitHub. Approve, request changes and inline comments are sent only
        after you confirm them; selecting a diff line opens the inline composer.
      </p>
    </div>
  {/if}
</section>

<style>
  .head-actions {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .search-panel {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 9px;
    margin-bottom: 7px;
    color: var(--text-faint);
  }

  .search {
    flex: 1;
    min-width: 0;
    height: 25px;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--font-small);
    outline: none;
  }

  .review-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(285px, 1fr));
    gap: 6px;
  }

  .repo-card {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    padding: 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .repo-card:hover {
    border-color: var(--accent);
    background: var(--bg-elev);
  }

  .repo-top {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .repo-icon {
    display: inline-flex;
    color: var(--text-faint);
  }

  .repo-name {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .open {
    display: inline-flex;
    color: var(--text-faint);
  }

  .repo-desc {
    font-size: 10px;
    line-height: 1.45;
    color: var(--text-muted);
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .facts {
    display: flex;
    gap: 9px;
    flex-wrap: wrap;
    font-size: 8.5px;
    color: var(--text-faint);
  }

  .review-cta {
    margin-top: 3px;
    font-size: 8.5px;
    color: var(--accent);
  }

  .note {
    margin-top: 8px;
    padding: 9px 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .note p {
    max-width: 90ch;
    font-size: 10px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    padding: 24px 18px;
  }

  .empty-title {
    font-size: var(--font-base);
    font-weight: 600;
  }

  .empty-copy {
    max-width: 68ch;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
    padding: 4px 1px;
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 10px;
  }

  .spin {
    display: inline-flex;
    animation: spin 900ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
