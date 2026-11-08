<script lang="ts">
  import { onMount } from "svelte";
  import { NAV_ITEMS } from "../nav";
  import { appearance, setDensity, setTheme, toggleSidebar } from "../state/appearance.svelte";
  import { resetDashboard } from "../state/dashboard.svelte";
  import { resetNav } from "../state/nav.svelte";
  import { ui, navigate } from "../state/ui.svelte";
  import { themes } from "../themes";
  import Icon from "./Icon.svelte";

  type Command = {
    id: string;
    label: string;
    group: string;
    hint: string;
    run: () => void;
  };

  const commands: Command[] = [
    ...NAV_ITEMS.map((item) => ({
      id: `go:${item.id}`,
      label: `Open ${item.label}`,
      group: "Navigate",
      hint: item.status === "planned" ? `Phase ${item.phase}` : "Ready",
      run: () => navigate(item.id),
    })),
    ...themes.map((theme) => ({
      id: `theme:${theme.id}`,
      label: `Theme: ${theme.name}`,
      group: "Appearance",
      hint: theme.blurb,
      run: () => setTheme(theme.id),
    })),
    {
      id: "density:compact",
      label: "Density: Compact",
      group: "Appearance",
      hint: "Tight rows, default",
      run: () => setDensity("compact"),
    },
    {
      id: "density:comfortable",
      label: "Density: Comfortable",
      group: "Appearance",
      hint: "More breathing room",
      run: () => setDensity("comfortable"),
    },
    {
      id: "density:roomy",
      label: "Density: Roomy",
      group: "Appearance",
      hint: "Largest spacing",
      run: () => setDensity("roomy"),
    },
    {
      id: "ui:sidebar",
      label: appearance.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar",
      group: "Interface",
      hint: "Ctrl B",
      run: () => toggleSidebar(),
    },
    {
      id: "dashboard:reset",
      label: "Reset dashboard layout",
      group: "Interface",
      hint: "Restore module order",
      run: () => resetDashboard(),
    },
    {
      id: "nav:reset",
      label: "Reset navigation preferences",
      group: "Interface",
      hint: "Show every module again",
      run: () => resetNav(),
    },
  ];

  let query = $state("");
  let index = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const results = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    const parts = q.split(/\s+/);
    return commands.filter((command) => {
      const haystack = `${command.label} ${command.group} ${command.hint}`.toLowerCase();
      return parts.every((part) => haystack.includes(part));
    });
  });

  $effect(() => {
    query;
    index = 0;
  });

  onMount(() => {
    inputEl?.focus();
  });

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (results.length) index = (index + 1) % results.length;
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      if (results.length) index = (index - 1 + results.length) % results.length;
    } else if (event.key === "Enter") {
      event.preventDefault();
      run(results[index]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      ui.paletteOpen = false;
    }
  }

  function run(command: Command | undefined): void {
    if (!command) return;
    ui.paletteOpen = false;
    command.run();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
<div
  class="overlay"
  role="dialog"
  aria-modal="true"
  aria-label="Command palette"
  tabindex="-1"
  onclick={(event) => {
    if (event.target === event.currentTarget) ui.paletteOpen = false;
  }}
>
  <div class="palette">
    <div class="input-row">
      <Icon name="search" size={14} />
      <input
        bind:this={inputEl}
        bind:value={query}
        placeholder="Search modules, themes and actions…"
        aria-label="Search commands"
        spellcheck="false"
        onkeydown={onKeydown}
      />
      <span class="kbd">Esc</span>
    </div>

    <ul class="results">
      {#each results as command, i (command.id)}
        <li>
          <button class="result" class:active={i === index} onclick={() => run(command)} onmouseenter={() => (index = i)}>
            <span class="group mono">{command.group}</span>
            <span class="label">{command.label}</span>
            <span class="hint mono">{command.hint}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No command matches “{query}”.</li>
      {/each}
    </ul>

    <div class="foot mono">
      <span>↑↓ navigate</span>
      <span>↵ run</span>
      <span>{results.length} results</span>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: color-mix(in srgb, var(--bg) 62%, transparent);
    backdrop-filter: blur(2px);
  }

  .palette {
    width: min(620px, calc(100vw - 40px));
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 60px -20px var(--shadow);
    overflow: hidden;
    animation: palette-in 110ms cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  @keyframes palette-in {
    from {
      opacity: 0;
      transform: translateY(-5px) scale(0.995);
    }
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 12px;
    height: 42px;
    border-bottom: 1px solid var(--border);
    color: var(--text-faint);
  }

  input {
    flex: 1;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
  }

  input::placeholder {
    color: var(--text-faint);
  }

  .results {
    max-height: 360px;
    overflow-y: auto;
    padding: 5px;
  }

  .result {
    display: grid;
    grid-template-columns: 82px 1fr auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 0 8px;
    height: 30px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--text-muted);
  }

  .result.active {
    background: var(--accent-soft);
    color: var(--text);
  }

  .group {
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .label {
    font-size: var(--font-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    font-size: 9.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .empty {
    padding: 18px 10px;
    color: var(--text-faint);
    font-size: var(--font-small);
    text-align: center;
  }

  .foot {
    display: flex;
    gap: 16px;
    padding: 6px 12px;
    border-top: 1px solid var(--border);
    font-size: 9.5px;
    color: var(--text-faint);
    background: var(--bg-elev);
  }

  .foot span:last-child {
    margin-left: auto;
  }
</style>
