<script lang="ts">
  import { onMount } from "svelte";
  import type { NavItem } from "../nav";
  import { openPalette } from "../state/ui.svelte";
  import { session } from "../state/session.svelte";
  import {
    clearNotifications,
    loadNotifications,
    markAllRead,
    notifications,
    openNotification,
    unreadCount,
  } from "../state/notifications.svelte";
  import { relativeTime } from "../format";
  import Icon from "./Icon.svelte";

  let { item }: { item: NavItem } = $props();
  let open = $state(false);
  const unread = $derived(unreadCount());

  onMount(() => {
    void loadNotifications();
  });

  function choose(notification: (typeof notifications.items)[number]): void {
    open = false;
    openNotification(notification);
  }
</script>

<header>
  <div class="crumbs">
    <span class="root mono">GRAFT</span>
    <span class="slash" aria-hidden="true">/</span>
    <span class="current">{item.label}</span>
    {#if item.status === "planned"}
      <span class="badge warn">Phase {item.phase}</span>
    {:else if item.id !== "settings"}
      <span class="badge accent">Live</span>
    {/if}
  </div>

  <div class="tools">
    <span class="engine mono-label" title={session.online ? "Rust engine connected" : "Engine unreachable"}>
      <span class="dot" class:on={session.online}></span>
      {session.online ? "engine online" : "engine offline"}
    </span>

    <div class="bell-wrap">
      <button
        class="bell"
        class:has-unread={unread > 0}
        aria-label={`Notifications${unread ? ` (${unread} unread)` : ""}`}
        aria-expanded={open}
        onclick={() => (open = !open)}
      >
        <Icon name="bell" size={14} />
        {#if unread > 0}<span class="unread">{unread > 99 ? "99+" : unread}</span>{/if}
      </button>

      {#if open}
        <div class="panel feed">
          <div class="feed-head">
            <span class="mono-label">notifications · {notifications.items.length}</span>
            <div class="feed-actions">
              <button class="link" onclick={markAllRead} disabled={unread === 0}>Mark read</button>
              <button class="link" onclick={clearNotifications} disabled={notifications.items.length === 0}>Clear</button>
            </div>
          </div>
          <div class="feed-list">
            {#each notifications.items as notification (notification.id)}
              <button
                class="feed-item"
                class:unread={!notification.read}
                onclick={() => choose(notification)}
              >
                <span class="tone {notification.tone}" aria-hidden="true"></span>
                <span class="feed-body">
                  <span class="feed-title">{notification.title}</span>
                  {#if notification.detail}<span class="feed-detail">{notification.detail}</span>{/if}
                  <span class="feed-time mono">{relativeTime(Math.floor(notification.at / 1000))}</span>
                </span>
                <Icon name="chevron-right" size={11} />
              </button>
            {:else}
              <p class="feed-empty">No notifications yet. Pushes, pulls, commits, checkouts, edits, forks and issues you trigger in GRAFT land here.</p>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <button class="cmd" onclick={openPalette} aria-label="Open command palette">
      <Icon name="search" size={13} />
      <span>Search or run a command</span>
      <span class="kbd">Ctrl K</span>
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    height: var(--topbar-h);
    padding: 0 12px 0 14px;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  .root {
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--text-faint);
  }

  .slash {
    color: var(--border-strong);
  }

  .current {
    font-size: var(--font-small);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }

  .engine {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9.5px;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--danger);
  }

  .dot.on {
    background: var(--ok);
  }

  .bell-wrap {
    position: relative;
  }

  .bell {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-faint);
    cursor: pointer;
  }

  .bell:hover,
  .bell.has-unread {
    color: var(--text);
    border-color: var(--border-strong);
  }

  .unread {
    position: absolute;
    top: -5px;
    right: -5px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 999px;
    background: var(--accent);
    color: var(--bg);
    font-family: var(--font-mono);
    font-size: 8px;
    line-height: 14px;
    text-align: center;
  }

  .feed {
    position: absolute;
    top: 32px;
    right: 0;
    z-index: 50;
    width: 360px;
    max-height: min(460px, 70vh);
    display: flex;
    flex-direction: column;
    box-shadow: 0 18px 40px -18px var(--shadow);
  }

  .feed-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 9px;
    border-bottom: 1px solid var(--border);
  }

  .feed-actions {
    display: flex;
    gap: 8px;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--text-faint);
    font: inherit;
    font-size: 9px;
    cursor: pointer;
  }

  .link:hover:not(:disabled) {
    color: var(--accent);
  }

  .link:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .feed-list {
    overflow-y: auto;
  }

  .feed-item {
    display: grid;
    grid-template-columns: 4px minmax(0, 1fr) auto;
    align-items: start;
    gap: 8px;
    width: 100%;
    padding: 8px 9px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    font: inherit;
  }

  .feed-item:hover {
    background: var(--bg-elev);
  }

  .feed-item.unread {
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }

  .tone {
    width: 3px;
    height: 100%;
    min-height: 20px;
    border-radius: 1px;
    background: var(--text-faint);
  }

  .tone.ok {
    background: var(--ok);
  }

  .tone.danger {
    background: var(--danger);
  }

  .feed-body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .feed-title {
    font-size: 10.5px;
    color: var(--text);
  }

  .feed-detail {
    font-size: 9.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .feed-time {
    font-size: 8.5px;
    color: var(--text-faint);
  }

  .feed-empty {
    padding: 16px 12px;
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
  }

  .cmd {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 7px 0 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-faint);
    font-size: 11.5px;
    cursor: pointer;
    transition:
      border-color var(--motion),
      color var(--motion),
      background var(--motion);
  }

  .cmd:hover {
    color: var(--text-muted);
    border-color: var(--border-strong);
    background: var(--surface-alt);
  }

  .cmd .kbd {
    margin-left: 4px;
  }

  @media (max-width: 1080px) {
    .cmd span:not(.kbd) {
      display: none;
    }
    .engine {
      display: none;
    }
  }
</style>
