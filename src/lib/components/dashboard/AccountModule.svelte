<script lang="ts">
  import { onMount } from "svelte";
  import PanelSection from "../PanelSection.svelte";
  import Icon from "../Icon.svelte";
  import {
    github,
    githubUser,
    isSignedIn,
    loadGithub,
  } from "../../state/github.svelte";
  import { navigate } from "../../state/ui.svelte";

  const user = $derived(githubUser());
  const signedIn = $derived(isSignedIn());
  const repoCount = $derived(
    user ? user.publicRepos + (user.totalPrivateRepos ?? 0) : null,
  );

  onMount(() => {
    if (!github.status && !github.loading) void loadGithub();
  });
</script>

<PanelSection title="GitHub Account" hint={signedIn ? "linked" : "identity"}>
  {#snippet action()}
    <span class="badge" class:ok={signedIn}>{signedIn ? "live" : "not linked"}</span>
  {/snippet}

  <div class="who">
    {#if user}
      <img class="avatar" src={user.avatarUrl} alt="" referrerpolicy="no-referrer" />
    {:else}
      <div class="avatar" aria-hidden="true"><Icon name="github" size={17} /></div>
    {/if}
    <div class="meta">
      <span class="login">{user?.login ?? (github.loading ? "Checking…" : "Not connected")}</span>
      <span class="mono-label">
        {#if user}
          {user.name ?? "github account"}
        {:else if github.status?.keyringError}
          keyring unavailable
        {:else}
          oauth device flow
        {/if}
      </span>
    </div>
  </div>

  <dl class="facts">
    <div><dt>Token storage</dt><dd>System keyring</dd></div>
    <div><dt>Scopes</dt><dd class="mono">{github.status?.scopes ?? "—"}</dd></div>
    <div><dt>Repositories</dt><dd class="mono">{repoCount ?? "—"}</dd></div>
  </dl>

  {#if github.status?.keyringError}
    <p class="note tone-danger">{github.status.keyringError}</p>
  {:else if github.status?.tokenError}
    <p class="note tone-danger">{github.status.tokenError}</p>
  {/if}

  <button class="btn primary connect" onclick={() => navigate("repositories")}>
    <Icon name="github" size={13} />
    {signedIn ? "Open repositories" : "Connect GitHub"}
  </button>
  <p class="note">
    {#if signedIn}
      The token lives in the Secret Service and is never rendered into the interface.
    {:else}
      Sign-in uses GitHub's device flow; the token is stored in the system keyring, never in SQLite.
    {/if}
  </p>
</PanelSection>

<style>
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }

  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    object-fit: cover;
    background: var(--bg-elev);
    color: var(--text-muted);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .login {
    font-size: var(--font-small);
    font-weight: 600;
  }

  .facts {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0;
    margin: 3px 0 10px;
  }

  .facts div {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 3px 0;
    font-size: var(--font-small);
    border-bottom: 1px dashed color-mix(in srgb, var(--border) 70%, transparent);
  }

  .facts div:last-child {
    border-bottom: 0;
  }

  dt {
    color: var(--text-faint);
  }

  dd {
    margin: 0;
    color: var(--text-muted);
  }

  .connect {
    width: 100%;
  }

  .note {
    margin-top: 8px;
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }
</style>
