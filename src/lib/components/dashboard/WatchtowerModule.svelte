<script lang="ts">
  import PanelSection from "../PanelSection.svelte";
</script>

<PanelSection title="Watchtower" hint="monitoring">
  {#snippet action()}
    <span class="badge warn">Phase 2</span>
  {/snippet}

  <div class="status">
    <span class="dot" aria-hidden="true"></span>
    <span class="label">Monitoring is off</span>
    <span class="mono-label">no worker running</span>
  </div>

  <dl class="counts">
    <div><dt>Watched repositories</dt><dd class="mono">0</dd></div>
    <div><dt>Unread alerts</dt><dd class="mono">0</dd></div>
    <div><dt>Poll interval</dt><dd class="mono">not set</dd></div>
  </dl>

  <p class="note">
    Watchtower will favourite any repository — yours or someone else's — and report releases,
    commits, pull requests, issues and workflow failures. Polling stays conditional and
    rate-limit aware, and the background worker is opt-in.
  </p>
</PanelSection>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 2px 0 9px;
    border-bottom: 1px solid var(--border);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-faint);
  }

  .label {
    font-size: var(--font-small);
    color: var(--text);
  }

  .counts {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin: 2px 0 8px;
  }

  .counts div {
    display: flex;
    justify-content: space-between;
    padding: 3px 0;
    font-size: var(--font-small);
  }

  dt {
    color: var(--text-faint);
  }

  dd {
    margin: 0;
    color: var(--text-muted);
    font-size: 10.5px;
  }

  .note {
    font-size: 10.5px;
    line-height: 1.5;
    color: var(--text-faint);
  }
</style>
