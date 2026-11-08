import { api, type TerminalEnvironment, type TerminalEvent } from "../api";
import type { TerminalSession } from "../terminal";
import { notify } from "./ui.svelte";

type LiveSession = TerminalSession;

type TerminalState = {
  env: TerminalEnvironment | null;
  loading: boolean;
  starting: boolean;
  error: string | null;
  sessions: LiveSession[];
  activeId: string | null;
};

const TRANSCRIPT_LIMIT = 1_500_000;
const listeners = new Map<string, Set<(event: TerminalEvent) => void>>();

export const terminal = $state<TerminalState>({
  env: null,
  loading: false,
  starting: false,
  error: null,
  sessions: [],
  activeId: null,
});

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

export async function loadTerminalEnvironment(): Promise<void> {
  if (terminal.loading) return;
  terminal.loading = true;
  try {
    terminal.env = await api.terminalEnvironment();
    terminal.error = null;
  } catch (cause) {
    terminal.error = message(cause);
  } finally {
    terminal.loading = false;
  }
}

function deliver(session: LiveSession, event: TerminalEvent): void {
  if (event.type === "output") {
    session.transcript += event.data;
    if (session.transcript.length > TRANSCRIPT_LIMIT) {
      session.transcript = session.transcript.slice(-TRANSCRIPT_LIMIT);
    }
  } else if (event.type === "exit") {
    session.ended = true;
  }
  if (!session.info.id) return;
  for (const listener of listeners.get(session.info.id) ?? []) listener(event);
}

export async function startTerminal(cwd: string, shell: string): Promise<string | null> {
  if (terminal.starting) return null;
  terminal.starting = true;
  terminal.error = null;
  const session: LiveSession = {
    info: { id: "", shell, cwd },
    transcript: "",
    ended: false,
    subscribe: () => () => undefined,
  };
  try {
    const info = await api.terminalStart(cwd, shell, 110, 30, (event) =>
      deliver(session, event),
    );
    session.info = info;
    terminal.sessions = [...terminal.sessions, session];
    terminal.activeId = info.id;
    return info.id;
  } catch (cause) {
    const text = message(cause);
    terminal.error = text;
    notify(text, "danger");
    return null;
  } finally {
    terminal.starting = false;
  }
}

export async function closeTerminal(id: string): Promise<void> {
  const index = terminal.sessions.findIndex((session) => session.info.id === id);
  if (index < 0) return;
  terminal.sessions = terminal.sessions.filter((session) => session.info.id !== id);
  listeners.delete(id);
  if (terminal.activeId === id) {
    terminal.activeId = terminal.sessions[Math.max(0, index - 1)]?.info.id ?? null;
  }
  try {
    await api.terminalClose(id);
  } catch (cause) {
    notify(message(cause), "danger");
  }
}

export function focusTerminal(id: string): void {
  terminal.activeId = id;
}

export function subscribeTerminal(
  id: string,
  listener: (event: TerminalEvent) => void,
): () => void {
  let set = listeners.get(id);
  if (!set) {
    set = new Set();
    listeners.set(id, set);
  }
  set.add(listener);
  return () => {
    set?.delete(listener);
    if (set && set.size === 0) listeners.delete(id);
  };
}
