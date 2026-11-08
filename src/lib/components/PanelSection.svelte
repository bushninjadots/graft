<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    hint?: string;
    action?: Snippet;
    children: Snippet;
  };

  let { title, hint, action, children }: Props = $props();
</script>

<section class="panel module">
  <header>
    <div class="heading">
      <h2>{title}</h2>
      {#if hint}<span class="mono-label">{hint}</span>{/if}
    </div>
    {#if action}
      <div class="action">{@render action()}</div>
    {/if}
  </header>
  <div class="body">{@render children()}</div>
</section>

<style>
  .module {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    height: calc(var(--row-h) + 8px);
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .heading {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  h2 {
    font-size: var(--font-small);
    font-weight: 600;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }

  .heading .mono-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .action {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }

  .body {
    padding: 10px 12px 12px;
    min-height: 0;
    flex: 1;
  }
</style>
