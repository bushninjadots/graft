<script lang="ts">
  import { session } from "../../state/session.svelte";
  import { plural } from "../../format";
  import PanelSection from "../PanelSection.svelte";
</script>

<PanelSection title="Activity" hint="local event log">
  <div class="empty">
    <p class="state">No local activity recorded yet.</p>
    <p class="note">
      GRAFT logs operations it performs — commits, fetches, extractions, project changes — as they
      happen. Nothing is imported or inferred, so this stays empty until the first Git operation
      lands in Phase 1.
    </p>
    <div class="facts mono-label">
      <span>database {session.online ? "ready" : "offline"}</span>
      <span>{plural(session.info ? session.info.schemaVersion : 0, "migration")} applied</span>
    </div>
  </div>
</PanelSection>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .state {
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  .note {
    font-size: 10.5px;
    line-height: 1.55;
    color: var(--text-faint);
    max-width: 70ch;
  }

  .facts {
    display: flex;
    gap: 14px;
    margin-top: 2px;
  }
</style>
