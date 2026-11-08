<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TopBar from "./lib/components/TopBar.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import Home from "./views/Home.svelte";
  import Projects from "./views/Projects.svelte";
  import Repositories from "./views/Repositories.svelte";
  import Repository from "./views/Repository.svelte";
  import ProfileStudio from "./views/ProfileStudio.svelte";
  import Watchtower from "./views/Watchtower.svelte";
  import TerminalView from "./views/Terminal.svelte";
  import Explore from "./views/Explore.svelte";
  import Collections from "./views/Collections.svelte";
  import Review from "./views/Review.svelte";
  import Extract from "./views/Extract.svelte";
  import SettingsView from "./views/Settings.svelte";
  import ModuleView from "./views/ModuleView.svelte";
  import { navById } from "./lib/nav";
  import { ui } from "./lib/state/ui.svelte";
  import { applyAppearance, loadAppearance, toggleSidebar } from "./lib/state/appearance.svelte";
  import { loadNav } from "./lib/state/nav.svelte";
  import { loadDashboard } from "./lib/state/dashboard.svelte";
  import { boot, refreshTools, session } from "./lib/state/session.svelte";
  import { syncAll } from "./lib/state/sync.svelte";
  import { loadWatchtower } from "./lib/state/watchtower.svelte";

  const active = $derived(
    navById(ui.view === "repository" ? "repositories" : ui.view) ?? navById("home")!,
  );

  onMount(() => {
    applyAppearance();

    void (async () => {
      await boot();
      await Promise.all([loadAppearance(), loadNav(), loadDashboard(), loadWatchtower()]);
      await refreshTools();
      if (session.online) void syncAll(true);
    })();

    let lastFocusSync = 0;
    const syncTimer = setInterval(() => {
      if (session.online && document.visibilityState === "visible") void syncAll();
    }, 90_000);

    function syncOnFocus(): void {
      if (!session.online || document.visibilityState !== "visible") return;
      if (Date.now() - lastFocusSync < 30_000) return;
      lastFocusSync = Date.now();
      void syncAll();
    }

    function syncOnVisibility(): void {
      if (document.visibilityState === "visible") syncOnFocus();
    }

    window.addEventListener("focus", syncOnFocus);
    document.addEventListener("visibilitychange", syncOnVisibility);

    function onKeydown(event: KeyboardEvent): void {
      const target = event.target as HTMLElement | null;
      const typing =
        !!target &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        ui.paletteOpen = !ui.paletteOpen;
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "b" && !typing) {
        event.preventDefault();
        toggleSidebar();
        return;
      }

      if (event.key === "Escape" && ui.paletteOpen) {
        event.preventDefault();
        ui.paletteOpen = false;
        return;
      }

      if (event.key === "/" && !typing && !ui.paletteOpen) {
        event.preventDefault();
        ui.paletteOpen = true;
      }
    }

    window.addEventListener("keydown", onKeydown);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("focus", syncOnFocus);
      document.removeEventListener("visibilitychange", syncOnVisibility);
      clearInterval(syncTimer);
    };
  });
</script>

<div class="shell">
  <Sidebar />

  <div class="main">
    <TopBar item={active} />

    {#if !session.online}
      <div class="offline">
        <span class="badge warn">backend offline</span>
        <span>{session.error ?? "The Rust engine is not reachable."}</span>
      </div>
    {/if}

    <main class="content">
      {#key ui.view}
        <div class="view-wrap" in:fade={{ duration: 90 }}>
          {#if ui.view === "home"}
            <Home />
          {:else if ui.view === "projects"}
            <Projects />
          {:else if ui.view === "terminal"}
            <TerminalView />
          {:else if ui.view === "repositories"}
            <Repositories />
          {:else if ui.view === "repository"}
            <Repository />
          {:else if ui.view === "profile"}
            <ProfileStudio />
          {:else if ui.view === "explore"}
            <Explore />
          {:else if ui.view === "collections"}
            <Collections />
          {:else if ui.view === "review"}
            <Review />
          {:else if ui.view === "extract"}
            <Extract />
          {:else if ui.view === "watchtower"}
            <Watchtower />
          {:else if ui.view === "settings"}
            <SettingsView />
          {:else}
            <ModuleView item={active} />
          {/if}
        </div>
      {/key}
    </main>

    <StatusBar />
  </div>

  {#if ui.paletteOpen}
    <CommandPalette />
  {/if}

  {#if ui.toast}
    {@const toast = ui.toast}
    {#if toast.action}
      <button
        class="toast clickable"
        class:danger={toast.tone === "danger"}
        class:ok={toast.tone === "ok"}
        onclick={toast.action}
      >
        {toast.text}
      </button>
    {:else}
      <div class="toast" class:danger={toast.tone === "danger"} class:ok={toast.tone === "ok"} role="status">
        {toast.text}
      </div>
    {/if}
  {/if}
</div>

<style>
  .shell {
    display: flex;
    height: 100%;
    min-height: 0;
    background: var(--bg);
  }

  .main {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .offline {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 14px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--warn) 7%, var(--bg));
    color: var(--text-muted);
    font-size: 10.5px;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .view-wrap {
    height: 100%;
  }

  .toast {
    position: fixed;
    font: inherit;
    text-align: left;
    right: 14px;
    bottom: calc(var(--statusbar-h) + 12px);
    z-index: 40;
    max-width: 380px;
    padding: 7px 11px;
    border: 1px solid var(--border-strong);
    border-left: 2px solid var(--accent);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: 0 14px 34px -14px var(--shadow);
    color: var(--text);
    font-size: 11px;
  }

  .toast.ok {
    border-left-color: var(--ok);
  }

  .toast.danger {
    border-left-color: var(--danger);
  }

  .toast.clickable {
    cursor: pointer;
  }

  .toast.clickable:hover {
    border-color: var(--border-strong);
  }
</style>
