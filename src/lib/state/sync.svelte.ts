import { syncGithub } from "./github.svelte";
import { pollGithubNotifications } from "./notifications.svelte";
import { pollWatchtower } from "./watchtower.svelte";

let busy = false;

/**
 * One sync pass shared by the focus listener, the interval and the TopBar
 * button. GitHub is refreshed first, then unread online notifications are
 * imported without marking them read on github.com.
 */
export async function syncAll(force = false): Promise<void> {
  if (busy) return;
  busy = true;
  try {
    await syncGithub(force);
    await pollGithubNotifications();
    await pollWatchtower(force);
  } finally {
    busy = false;
  }
}
