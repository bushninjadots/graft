<script lang="ts">
  import PanelSection from "../PanelSection.svelte";

  const shortcuts = [
    { keys: ["Ctrl", "K"], action: "Command palette" },
    { keys: ["Ctrl", "B"], action: "Collapse sidebar" },
    { keys: ["↑", "↓"], action: "Move through results" },
    { keys: ["↵"], action: "Run selection" },
    { keys: ["Esc"], action: "Close overlay" },
  ];
</script>

<PanelSection title="Shortcuts" hint="keyboard">
  <ul>
    {#each shortcuts as shortcut (shortcut.action)}
      <li>
        <span class="action">{shortcut.action}</span>
        <span class="keys">
          {#each shortcut.keys as key (key)}
            <span class="kbd">{key}</span>
          {/each}
        </span>
      </li>
    {/each}
  </ul>
</PanelSection>

<style>
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    height: calc(var(--row-h) - 1px);
  }

  .action {
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .keys {
    display: inline-flex;
    gap: 3px;
  }
</style>
