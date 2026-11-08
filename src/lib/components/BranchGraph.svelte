<script lang="ts">
  import type { RepoCommit } from "../api";

  type GraphRow = {
    commit: RepoCommit;
    lane: number;
    before: (string | null)[];
    after: (string | null)[];
    parents: number[];
  };

  type Props = {
    commits: RepoCommit[];
    onselect: (sha: string) => void;
  };

  let { commits, onselect }: Props = $props();

  function graphRows(list: RepoCommit[]): GraphRow[] {
    const lanes: (string | null)[] = [];
    const rows: GraphRow[] = [];
    for (const commit of list) {
      let lane = lanes.indexOf(commit.sha);
      if (lane < 0) {
        lane = lanes.findIndex((value) => !value);
        if (lane < 0) {
          lane = lanes.length;
          lanes.push(commit.sha);
        } else {
          lanes[lane] = commit.sha;
        }
      }
      const before = [...lanes];
      const parents = commit.parents ?? [];
      const parentLanes: number[] = [];
      lanes[lane] = parents[0] ?? null;
      for (const parent of parents.slice(1)) {
        let index = lanes.indexOf(parent);
        if (index < 0) {
          index = lanes.findIndex((value) => !value);
          if (index < 0) {
            index = lanes.length;
            lanes.push(parent);
          } else {
            lanes[index] = parent;
          }
        }
        parentLanes.push(index);
      }
      if (parents.length === 0) lanes[lane] = null;
      while (lanes.length > 0 && lanes[lanes.length - 1] === null) lanes.pop();
      rows.push({
        commit,
        lane,
        before,
        after: [...lanes],
        parents: parentLanes,
      });
    }
    return rows;
  }

  const rows = $derived(graphRows(commits));
  const lanes = $derived(
    Math.max(
      2,
      ...rows.map((row) =>
        Math.max(row.before.length, row.after.length, row.lane + 1, ...row.parents.map((p) => p + 1)),
      ),
    ),
  );
  const width = $derived(lanes * 11);

  function x(lane: number): number {
    return 7 + lane * 11;
  }
</script>

{#if rows.length === 0}
  <p class="state">No commits on this branch.</p>
{:else}
  <div class="graph-wrap" style={`--graph-width: ${width}px`}>
    {#each rows as row (row.commit.sha)}
      <button class="graph-row" onclick={() => onselect(row.commit.sha)} title={`Inspect ${row.commit.short}`}>
        <span class="graph" style={`width: ${width}px`}>
          <svg viewBox={`0 0 ${width} 28`} width={width} height="28" aria-hidden="true">
            {#each row.before as _sha, index}
              {#if _sha}
                <line x1={x(index)} y1="0" x2={x(index)} y2="14" class="lane" />
              {/if}
            {/each}
            {#each row.parents as parent, index}
              <path
                d={`M ${x(row.lane)} 14 C ${x(row.lane)} 24, ${x(parent)} 4, ${x(parent)} 28`}
                class:merge={index > 0}
                class="lane edge"
              />
            {/each}
            {#each row.after as _sha, index}
              {#if _sha}
                <line x1={x(index)} y1="14" x2={x(index)} y2="28" class="lane" />
              {/if}
            {/each}
            <circle cx={x(row.lane)} cy="14" r="3" class="node" />
          </svg>
        </span>
        <span class="sha mono">{row.commit.short}</span>
        <span class="msg">{row.commit.message}</span>
        {#if (row.commit.parents ?? []).length > 1}<span class="badge">merge</span>{/if}
        <span class="who mono faint">{row.commit.authorLogin ?? row.commit.authorName}</span>
        <span class="when mono faint">{row.commit.date ? row.commit.date.slice(0, 10) : ""}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .graph-wrap {
    display: flex;
    flex-direction: column;
  }

  .graph-row {
    display: grid;
    grid-template-columns: var(--graph-width) 48px minmax(0, 1fr) auto 88px 74px;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 28px;
    padding: 2px 8px 2px 4px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .graph-row:hover {
    background: var(--bg-elev);
  }

  .graph {
    display: flex;
    align-items: center;
    overflow: hidden;
  }

  .lane {
    stroke: var(--border-strong);
    stroke-width: 1.15;
    fill: none;
  }

  .lane.edge {
    opacity: 0.85;
  }

  .lane.merge {
    stroke: var(--accent);
    stroke-dasharray: 2 2;
  }

  .node {
    fill: var(--accent);
    stroke: var(--surface);
    stroke-width: 1.5;
  }

  .sha {
    font-size: 9px;
    color: var(--text-faint);
  }

  .msg {
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .who,
  .when {
    font-size: 8.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    font-size: 7.5px;
    padding: 0 4px;
  }

  .state {
    padding: 12px;
    font-size: var(--font-small);
    color: var(--text-muted);
  }

  @media (max-width: 900px) {
    .graph-row {
      grid-template-columns: var(--graph-width) 44px minmax(0, 1fr) auto;
    }

    .who,
    .when {
      display: none;
    }
  }
</style>
