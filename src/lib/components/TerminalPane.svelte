<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { api, type TerminalEvent } from "../api";
  import { subscribeTerminal } from "../state/terminal.svelte";
  import type { TerminalSession } from "../terminal";

  let { session, active }: { session: TerminalSession; active: boolean } = $props();
  let host = $state<HTMLDivElement | null>(null);
  let terminal: Terminal | null = null;
  let fitAddon: FitAddon | null = null;

  function token(styles: CSSStyleDeclaration, name: string, fallback: string): string {
    return styles.getPropertyValue(name).trim() || fallback;
  }

  onMount(() => {
    if (!host) return;
    const styles = getComputedStyle(document.documentElement);
    const term = new Terminal({
      cursorBlink: true,
      cursorStyle: "bar",
      fontFamily: token(styles, "--font-mono", "monospace"),
      fontSize: 11.5,
      lineHeight: 1.25,
      scrollback: 5000,
      allowProposedApi: true,
      theme: {
        background: token(styles, "--bg-elev", "#0f1213"),
        foreground: token(styles, "--text", "#e2e6e6"),
        cursor: token(styles, "--accent", "#d9a441"),
        selectionBackground: token(styles, "--accent-soft", "rgba(217,164,65,.2)"),
        black: token(styles, "--bg", "#0b0d0e"),
        red: token(styles, "--danger", "#d16a5f"),
        green: token(styles, "--ok", "#79b47a"),
        yellow: token(styles, "--warn", "#d9a441"),
        blue: token(styles, "--info", "#7ba7c4"),
        magenta: token(styles, "--accent", "#d9a441"),
        cyan: token(styles, "--info", "#7ba7c4"),
        white: token(styles, "--text", "#e2e6e6"),
        brightBlack: token(styles, "--text-faint", "#6a7374"),
        brightRed: token(styles, "--danger", "#d16a5f"),
        brightGreen: token(styles, "--ok", "#79b47a"),
        brightYellow: token(styles, "--warn", "#d9a441"),
        brightBlue: token(styles, "--info", "#7ba7c4"),
        brightMagenta: token(styles, "--accent", "#d9a441"),
        brightCyan: token(styles, "--info", "#7ba7c4"),
        brightWhite: token(styles, "--text", "#e2e6e6"),
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    terminal = term;
    fitAddon = fit;

    const write = (event: TerminalEvent) => {
      if (event.type === "output") {
        term.write(event.data);
      } else if (event.type === "error") {
        term.write(`\r\n\x1b[31m${event.message}\x1b[0m\r\n`);
      } else {
        term.write("\r\n\x1b[90m[process exited]\x1b[0m\r\n");
      }
    };
    if (session.transcript) term.write(session.transcript);
    if (session.ended) term.write("\r\n\x1b[90m[process exited]\x1b[0m\r\n");
    const unsubscribe = subscribeTerminal(session.info.id, write);
    const dataSub = term.onData((data) => {
      void api.terminalWrite(session.info.id, data).catch(() => undefined);
    });
    const resizeSub = term.onResize(({ cols, rows }) => {
      void api.terminalResize(session.info.id, cols, rows).catch(() => undefined);
    });

    requestAnimationFrame(() => {
      fit.fit();
      term.focus();
    });

    return () => {
      unsubscribe();
      dataSub.dispose();
      resizeSub.dispose();
      term.dispose();
      terminal = null;
      fitAddon = null;
    };
  });

  $effect(() => {
    if (!active || !terminal) return;
    void tick().then(() => {
      fitAddon?.fit();
      terminal?.focus();
    });
  });
</script>

<div class="pane" bind:this={host} class:hidden={!active}></div>

<style>
  .pane {
    height: 100%;
    min-height: 0;
    padding: 7px 8px 4px;
    background: var(--bg-elev);
  }

  .pane.hidden {
    display: none;
  }

  .pane :global(.xterm) {
    height: 100%;
  }

  .pane :global(.xterm-viewport) {
    background: transparent !important;
    scrollbar-color: var(--border) transparent;
  }
</style>
