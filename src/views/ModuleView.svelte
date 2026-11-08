<script lang="ts">
  import type { NavItem } from "../lib/nav";
  import Icon from "../lib/components/Icon.svelte";

  let { item }: { item: NavItem } = $props();
</script>

<section class="view">
  <div class="view-head">
    <div>
      <h1 class="view-title">{item.label}</h1>
      <p class="view-sub">{item.summary}</p>
    </div>
    <span class="badge warn">Phase {item.phase}</span>
  </div>

  <div class="panel stage">
    <div class="stage-head">
      <span class="badge">Planned</span>
      <span class="mono-label">module id · {item.id}</span>
    </div>

    <ul class="bullets">
      {#each item.bullets as bullet (bullet)}
        <li>
          <span class="marker"><Icon name="dot" size={10} /></span>
          <span>{bullet}</span>
        </li>
      {/each}
    </ul>

    <div class="rule"></div>

    <p class="honest">
      This module has no implementation yet, so GRAFT shows no placeholder data for it. The shell
      around it — navigation, theming, layout persistence and settings — is real. The engine for
      this surface is scheduled for Phase {item.phase}; see <span class="mono">docs/PLAN.md</span>.
    </p>
  </div>
</section>

<style>
  .stage {
    padding: 14px 16px 16px;
    max-width: 860px;
  }

  .stage-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }

  .bullets {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 0;
  }

  .bullets li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--font-small);
    color: var(--text-muted);
    line-height: 1.5;
  }

  .marker {
    display: inline-flex;
    margin-top: 4px;
    color: var(--accent);
  }

  .rule {
    height: 1px;
    background: var(--border);
  }

  .honest {
    margin-top: 12px;
    font-size: 10.5px;
    line-height: 1.6;
    color: var(--text-faint);
    max-width: 72ch;
  }

  .honest .mono {
    color: var(--text-muted);
  }
</style>
