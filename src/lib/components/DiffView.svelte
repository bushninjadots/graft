<script lang="ts">
  import type { RepoPullComment } from "../api";

  type DiffFile = {
    filename: string;
    status: string;
    additions: number;
    deletions: number;
    patch: string | null;
  };

  type DiffLine = {
    kind: "hunk" | "same" | "add" | "del";
    text: string;
    oldNo: number | null;
    newNo: number | null;
  };

  type SideRow = {
    left: DiffLine | null;
    right: DiffLine | null;
    hunk?: boolean;
  };

  type Props = {
    files: DiffFile[];
    comments?: RepoPullComment[];
    commentable?: boolean;
    oncomment?: (path: string, line: number, side: "LEFT" | "RIGHT") => void;
  };

  let { files, comments = [], commentable = false, oncomment }: Props = $props();
  let mode = $state<"unified" | "side">("unified");

  function parsePatch(patch: string | null): DiffLine[] {
    if (!patch) return [];
    const lines = patch.split("\n");
    const out: DiffLine[] = [];
    let oldNo = 0;
    let newNo = 0;
    for (const raw of lines) {
      const hunk = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/);
      if (hunk) {
        oldNo = Number(hunk[1]);
        newNo = Number(hunk[2]);
        out.push({
          kind: "hunk",
          text: `${raw.slice(0, raw.indexOf("@@", 2) + 2)}${hunk[3] ?? ""}`.trim(),
          oldNo: null,
          newNo: null,
        });
        continue;
      }
      if (raw.startsWith("\\")) continue;
      if (raw.startsWith("+")) {
        out.push({ kind: "add", text: raw.slice(1), oldNo: null, newNo: newNo++ });
      } else if (raw.startsWith("-")) {
        out.push({ kind: "del", text: raw.slice(1), oldNo: oldNo++, newNo: null });
      } else if (raw.startsWith(" ")) {
        out.push({ kind: "same", text: raw.slice(1), oldNo: oldNo++, newNo: newNo++ });
      } else if (raw === "") {
        out.push({ kind: "same", text: "", oldNo: oldNo++, newNo: newNo++ });
      }
    }
    return out;
  }

  function sideRows(patch: string | null): SideRow[] {
    const lines = parsePatch(patch);
    const out: SideRow[] = [];
    let index = 0;
    while (index < lines.length) {
      const line = lines[index];
      if (line.kind === "hunk") {
        out.push({ left: line, right: line, hunk: true });
        index += 1;
        continue;
      }
      if (line.kind === "same") {
        out.push({ left: line, right: line });
        index += 1;
        continue;
      }
      const removals: DiffLine[] = [];
      const additions: DiffLine[] = [];
      while (
        index < lines.length &&
        (lines[index].kind === "add" || lines[index].kind === "del")
      ) {
        if (lines[index].kind === "add") additions.push(lines[index]);
        else removals.push(lines[index]);
        index += 1;
      }
      const count = Math.max(removals.length, additions.length);
      for (let row = 0; row < count; row += 1) {
        out.push({ left: removals[row] ?? null, right: additions[row] ?? null });
      }
    }
    return out;
  }

  function statusLabel(status: string): string {
    return (
      {
        added: "added",
        removed: "deleted",
        modified: "modified",
        renamed: "renamed",
      }[status] ?? status ?? "changed"
    );
  }

  function fileComments(file: DiffFile): RepoPullComment[] {
    return comments.filter((comment) => comment.path === file.filename);
  }

  function lineComments(file: DiffFile, line: number, side: "LEFT" | "RIGHT"): RepoPullComment[] {
    return comments.filter(
      (comment) =>
        comment.path === file.filename &&
        (comment.line ?? comment.originalLine) === line &&
        (comment.side ?? "RIGHT") === side,
    );
  }

  function comment(path: string, line: number | null, side: "LEFT" | "RIGHT"): void {
    if (!commentable || !line) return;
    oncomment?.(path, line, side);
  }
</script>

<div class="diff-toolbar">
  <span class="mono-label">{files.length} file{files.length === 1 ? "" : "s"} changed</span>
  <div class="segmented">
    <button class="seg" class:active={mode === "unified"} onclick={() => (mode = "unified")}>Unified</button>
    <button class="seg" class:active={mode === "side"} onclick={() => (mode = "side")}>Side by side</button>
  </div>
</div>

{#each files as file (file.filename)}
  {@const patchRows = parsePatch(file.patch)}
  {@const pairedRows = sideRows(file.patch)}
  {@const commentsForFile = fileComments(file)}
  <section class="diff-file">
    <header class="diff-head">
      <span class="status {file.status}">{statusLabel(file.status)}</span>
      <span class="filename mono">{file.filename}</span>
      <span class="counts mono">
        <span class="add">+{file.additions}</span>
        <span class="del">−{file.deletions}</span>
      </span>
    </header>

    {#if commentsForFile.length > 0}
      <div class="file-comments">
        {#each commentsForFile as comment (comment.id)}
          <div class="file-comment">
            <span class="comment-meta mono">
              @{comment.authorLogin}
              {#if comment.path && (comment.line ?? comment.originalLine)}
                · {comment.path}:{comment.line ?? comment.originalLine} ({comment.side ?? "RIGHT"})
              {/if}
            </span>
            <p>{comment.body}</p>
          </div>
        {/each}
      </div>
    {/if}

    {#if !file.patch}
      <p class="empty">No textual diff — binary or too large for the GitHub patch response.</p>
    {:else if mode === "unified"}
      <div class="patch unified">
        {#each patchRows as line, index (`${index}-${line.kind}`)}
          {#if line.kind === "hunk"}
            <div class="row hunk">
              <span class="no"></span>
              <span class="no"></span>
              <span class="code">{line.text}</span>
            </div>
          {:else}
            <div class="row {line.kind}">
              {#if line.oldNo}
                <button
                  class="no gutter"
                  class:active={commentable}
                  title={commentable ? `Comment on ${file.filename}:${line.oldNo} (left)` : undefined}
                  onclick={() => comment(file.filename, line.oldNo, "LEFT")}
                  disabled={!commentable}
                >{line.oldNo}</button>
              {:else}
                <span class="no"></span>
              {/if}
              {#if line.newNo}
                <button
                  class="no gutter"
                  class:active={commentable}
                  title={commentable ? `Comment on ${file.filename}:${line.newNo} (right)` : undefined}
                  onclick={() => comment(file.filename, line.newNo, "RIGHT")}
                  disabled={!commentable}
                >{line.newNo}</button>
              {:else}
                <span class="no"></span>
              {/if}
              <span class="sign">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>
              <span class="code">{line.text || " "}</span>
            </div>
            {#each lineComments(file, line.newNo ?? line.oldNo ?? 0, line.newNo ? "RIGHT" : "LEFT") as comment (comment.id)}
              <div class="inline-comment">
                <span class="mono comment-meta">@{comment.authorLogin}</span>
                <p>{comment.body}</p>
              </div>
            {/each}
          {/if}
        {/each}
      </div>
    {:else}
      <div class="patch side">
        {#each pairedRows as row, index (`${index}-side`)}
          {#if row.hunk}
            <div class="side-hunk">
              <span></span><span class="code">{row.right?.text ?? ""}</span>
              <span></span><span></span>
            </div>
          {:else}
            <div class="side-row">
              <button
                class="no"
                class:active={commentable && !!row.left?.oldNo}
                title={commentable && row.left?.oldNo ? `Comment on ${file.filename}:${row.left.oldNo} (left)` : undefined}
                onclick={() => comment(file.filename, row.left?.oldNo ?? null, "LEFT")}
                disabled={!commentable || !row.left?.oldNo}
              >{row.left?.oldNo ?? ""}</button>
              <span class="code {row.left?.kind ?? "blank"}">{row.left?.text ?? ""}</span>
              <button
                class="no"
                class:active={commentable && !!row.right?.newNo}
                title={commentable && row.right?.newNo ? `Comment on ${file.filename}:${row.right.newNo} (right)` : undefined}
                onclick={() => comment(file.filename, row.right?.newNo ?? null, "RIGHT")}
                disabled={!commentable || !row.right?.newNo}
              >{row.right?.newNo ?? ""}</button>
              <span class="code {row.right?.kind ?? "blank"}">{row.right?.text ?? ""}</span>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </section>
{/each}

<style>
  .diff-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 1px 7px;
  }

  .segmented {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .seg {
    height: 21px;
    padding: 0 8px;
    border: 0;
    border-right: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-faint);
    font: inherit;
    font-size: 8.5px;
    cursor: pointer;
  }

  .seg:last-child {
    border-right: 0;
  }

  .seg.active {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .diff-file {
    margin-bottom: 7px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg);
  }

  .diff-head {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 8px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .status {
    padding: 1px 5px;
    border-radius: 999px;
    border: 1px solid var(--border);
    font-size: 7.5px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }

  .status.added {
    color: var(--ok);
    border-color: color-mix(in srgb, var(--ok) 35%, var(--border));
  }

  .status.removed {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 35%, var(--border));
  }

  .status.modified {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 35%, var(--border));
  }

  .filename {
    flex: 1;
    min-width: 0;
    font-size: 9.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .counts {
    display: flex;
    gap: 6px;
    font-size: 8.5px;
  }

  .add {
    color: var(--ok);
  }

  .del {
    color: var(--danger);
  }

  .patch {
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.55;
    overflow-x: auto;
  }

  .unified {
    min-width: 100%;
    width: max-content;
  }

  .row {
    display: grid;
    grid-template-columns: 36px 36px 14px minmax(0, 1fr);
    min-height: 15px;
    white-space: pre;
  }

  .row.same {
    background: transparent;
  }

  .row.add {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .row.del {
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .row.hunk {
    grid-template-columns: 72px minmax(0, 1fr);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    color: var(--text-faint);
  }

  .row.hunk .code {
    padding: 2px 8px;
  }

  .row .no {
    padding: 0 5px 0 3px;
    border: 0;
    background: color-mix(in srgb, var(--border) 30%, transparent);
    color: var(--text-faint);
    font: inherit;
    text-align: right;
    user-select: none;
  }

  .row .no.gutter {
    cursor: default;
  }

  .row .no.gutter.active {
    cursor: pointer;
  }

  .row .no.gutter.active:hover {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .row .sign {
    color: var(--text-faint);
    text-align: center;
    user-select: none;
  }

  .row .code {
    padding: 0 8px 0 2px;
    color: var(--text);
  }

  .side {
    display: grid;
    grid-template-columns: 1fr 1fr;
    width: 100%;
  }

  .side-row {
    display: contents;
  }

  .side-row .no {
    width: 36px;
    padding: 0 5px;
    border: 0;
    border-right: 1px solid color-mix(in srgb, var(--border) 50%, transparent);
    background: color-mix(in srgb, var(--border) 22%, transparent);
    color: var(--text-faint);
    font: inherit;
    text-align: right;
    cursor: default;
  }

  .side-row .no.active {
    cursor: pointer;
  }

  .side-row .no.active:hover {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .side-row .code {
    display: block;
    min-height: 15px;
    padding: 0 6px;
    border-right: 1px solid var(--border);
    white-space: pre;
    overflow: hidden;
  }

  .side-row .code.add {
    background: color-mix(in srgb, var(--ok) 9%, transparent);
  }

  .side-row .code.del {
    background: color-mix(in srgb, var(--danger) 9%, transparent);
  }

  .side-row .code.blank {
    background: repeating-linear-gradient(
      -45deg,
      transparent,
      transparent 4px,
      color-mix(in srgb, var(--border) 22%, transparent) 4px,
      color-mix(in srgb, var(--border) 22%, transparent) 5px
    );
  }

  .side-hunk {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: 36px 1fr;
    padding: 2px 0;
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    color: var(--text-faint);
  }

  .side-hunk span {
    padding: 0 6px;
  }

  .file-comments {
    padding: 5px 8px;
    background: color-mix(in srgb, var(--warn) 4%, transparent);
    border-bottom: 1px solid var(--border);
  }

  .file-comment {
    padding: 3px 0;
  }

  .comment-meta {
    font-size: 8.5px;
    color: var(--text-faint);
  }

  .file-comment p,
  .inline-comment p {
    margin: 2px 0 0;
    font-size: 10px;
    line-height: 1.5;
    color: var(--text-muted);
    white-space: pre-wrap;
  }

  .inline-comment {
    margin-left: 86px;
    padding: 4px 8px;
    border-left: 2px solid var(--accent);
    background: var(--surface);
  }

  .empty {
    padding: 10px;
    font-size: 10px;
    color: var(--text-faint);
  }
</style>
