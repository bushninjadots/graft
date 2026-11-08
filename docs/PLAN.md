# GRAFT — build plan

Living document. Update it when scope, decisions or phase status change.

Product spec: `docs/SPEC.md` (the blueprint this plan implements).
Architecture: `docs/ARCHITECTURE.md`. Visual system: `docs/DESIGN.md`.

---

## Current status

**Phase 1–3 complete for the shipped surfaces.** Phase 1a added auth, the repository
list and local Git inspection. Phase 1b added the in-app repository workspace
(README, code tree, commits, pull requests, issues, branches), star/fork/clone, and
fetch/pull/push for linked projects. Phase 1 also ships the real PTY terminal
(`portable-pty` + xterm.js) with project working directories and persistent tabs.
Phase 2 ships Watchtower: a radar for GitHub stars, local favourites, frequently
visited repositories and similar recommendations derived from languages and topics,
plus ETag-conditional polling, eight alert categories, local SQLite history, native
notifications when hidden and opt-in throttled background monitoring. Phase 3 adds unified/side-by-side diffs, approvals,
change requests and inline review comments, a commit branch graph, a standalone
Explore surface with saved searches, and Collections with local statuses and notes.
It also includes Profile Studio (live snapshot, presets, pins, stats, activity map,
achievements, organisations, local draft, explicit publish), repository text-file
editing with push-access checks, issue listing/creation, three repository layouts,
live profile status editing with device-flow scope upgrade, and focus/timer sync
with unread GitHub notification import. Phase 4 adds Extract Studio: a read-only
repository scanner with language adapters, import-graph analysis, component
suggestions, pre-flight extraction plans and confirmed copy, move and history-split
strategies with backups and a recorded run log. Phase 0 is complete and verified as
well.

Delivered in Phase 1a:

| # | Phase 1 requirement | State |
| - | ------------------- | ----- |
| 1 | GitHub OAuth device flow from Rust | done |
| 2 | Token storage in the Secret Service keyring | done |
| 3 | Personal access token alternative, verified before storage | done |
| 4 | Repository list (public/private, organisations), filter, pagination | done |
| 5 | Account panel fed by the real API, with cached profile | done |
| 6 | Local project Git state (branch, changes, ahead/behind, remote) | done |
| 7 | GitHub settings (client ID, status, sign out) | done |
| 8 | Repository workspace: README, file tree, file text, branches | done |
| 9 | Commit history with per-file diffs | done |
| 10 | Pull requests with body and file diffs | done |
| 11 | Star/unstar, fork, clone/download, clone-to-project | done |
| 12 | Local project fetch / pull / push | done |
| 13 | Commit and branch write operations | done (with confirmations) |

Still planned: rate-limit surfacing beyond error messages and releases. ETag caching
now ships in Watchtower; repository discovery is available both inside Repositories
and as the standalone Explore surface.

### Verification (Phase 1a)

| Check | Result |
| ----- | ------ |
| `svelte-check` | 0 errors, 0 warnings |
| `vite build` | passes — 330 kB JS / 67 kB CSS (builder, search, sync, notifications and issues included) |
| `cargo check` | passes |
| `cargo test` | 30/30 pass (migrations, settings, projects, Git parsing, branch validation, a real commit/branch cycle in a temp repo, device flow, HTTP client, profile publish/write payloads, profile snapshot shape and every README section, issues, file-path validation, search queries, notification mapping, PTY validation, Watchtower parsing/history/visits) |
| Repository API | verified live with a real token: repository list, detail, README and branches rendered in the running window; commits and pull requests fetched with no errors |
| Keyring round-trip | verified against the live Secret Service on this machine |
| Visual review | running window captured with `grim`; Repositories sign-in flow inspected |
| Live GitHub calls | verified end to end by seeding the existing `gh` token into the keyring; device flow itself still needs a client ID |

Delivered in this milestone:

| # | Phase 0 requirement | State |
| - | ------------------- | ----- |
| 1 | Working Tauri 2 + Rust + Svelte 5 application | done |
| 2 | Clean project architecture (`src/lib/*`, `src/views/*`, `src-tauri/src/*`) | done |
| 3 | Polished initial interface (custom shell, not a Tauri starter) | done |
| 4 | Functional navigation (groups, pin, hide, reorder, command palette) | done |
| 5 | Theme engine, 7 themes, density and radius controls | done |
| 6 | Configurable dashboard layout (drag, hide, persist) | done |
| 7 | SQLite with versioned migrations (`user_version`) | done |
| 8 | Settings system (appearance, navigation, dashboard, data) | done |
| 9 | Dev/build scripts (`app:dev`, `app:build`, `verify`) | done |
| 10 | Documentation (README, architecture, design, plan) | done |

Not in Phase 0: GitHub auth, network access, git operations, terminal, extraction,
AI, plugins. Every unimplemented module renders an explicit status panel — it does
not show fake data.

### Verification (Phase 0)

| Check | Result |
| ----- | ------ |
| `svelte-check` (types + templates) | 0 errors, 0 warnings |
| `vite build` (production bundle) | passes — 119 kB JS / 30 kB CSS |
| `cargo check` | passes |
| `cargo test` (migration idempotency, settings roundtrip, project lifecycle) | 3/3 pass |
| `npm run app:dev` launch on Wayland | binary starts and stays up; Vite + Tauri dev pipeline clean |

---

## Environment (verified)

- EndeavourOS / Arch, kernel 7.2, Wayland session.
- rustc + cargo 1.98.1, Node 26.13, npm 12.2.
- webkit2gtk-4.1 2.54.2, javascriptcoregtk-4.1, libsoup-3.0, gtk3 3.24.52 present.
- git 2.56, gh, fish/bash/zsh, sqlite3, patchelf present.
- Fonts available: JetBrains Mono (system), so the mono stack is native.

---

## Decisions taken

1. **One folder per concern, no monorepo split.** `src/` (UI) and `src-tauri/` (engine)
   in a single repository, as Tauri intends. Plugins get their own crates in
   `src-tauri/plugins/` when Phase 6 starts.
2. **rusqlite (bundled SQLite).** No system SQLite version drift; the bundled build
   is slower to compile once and then stable. `user_version` drives migrations.
3. **Settings are JSON rows, not a column per preference.** Unknown keys survive
   upgrades; a corrupt blob falls back to defaults instead of breaking boot.
4. **Network access is Rust-only.** The webview CSP still allows no external
   connections (`connect-src` is IPC only). GitHub requests use `reqwest` with
   native TLS; avatar images are the single allowed remote image origin.
5. **Secrets go to the Secret Service.** `keyring` 3 with `sync-secret-service`;
   if no provider is running GRAFT reports the failure instead of writing a token to
   disk. The OAuth client ID is public and lives in the settings table.
6. **Read-only Git inspection.** `git status`/`remote`/`rev-parse` only; no write
   operation exists until the Git workspace phase lands with confirmations.
5. **Frontend failures are visible.** If the Rust backend is unreachable (browser
   preview), the UI shows an offline banner — it never pretends operations succeeded.
6. **Theme tokens are the only colour source.** Components use CSS variables; the
   theme engine writes them to `:root`. No component-level hex values.
7. **`docs/SPEC.md` is authoritative for scope**, and `docs/PLAN.md` for order.
8. **Device flow over an embedded webview login.** No GitHub password or cookie ever
   passes through GRAFT; the user approves the app on github.com.

---

## Phase 1 — Core GRAFT (complete)

Goal: useful every day without GitHub being a mock.

1. **Auth foundation**
   - GitHub OAuth device flow via a Rust HTTP client (`reqwest` + `rustls`).
   - Token storage in the Secret Service keyring (`secret-service` / `oo7`),
     never in SQLite or localStorage. Fallback: encrypted file with a warning.
   - Scopes: `read:user`, `repo` only when private access is requested.
2. **API client layer**
   - `src-tauri/src/github/` with a typed client, ETag caching, pagination helper,
     rate-limit headers surfaced to the UI, and a request log for debugging.
3. **Account + repositories**
   - Account panel becomes real: avatar, login, scopes, rate limit.
   - Repository list with search, sort, filters; repository detail with README,
     languages, branches, recent commits.
4. **Local project detection**
   - `git` CLI shell-out (`src-tauri/src/git/`), no libgit2 yet.
   - Detect status/branch/ahead-behind/remote for a chosen directory only.
5. **Git workspace v1**
   - Fetch, pull, push, commit, branch switch/create, diff view (read + write ops
     with confirmation and a dry-run summary).
6. **Terminal v1**
   - `portable-pty` + `xterm.js`, fish default, multi-session, project cwd.
7. **Settings additions**
   - GitHub account, keyring status, preferred editor/terminal, monitoring interval.

Exit criteria: a user can sign in, see their repositories, import a local project,
run git operations and open a terminal — all with real data.

---

## Phase 2 — Watchtower

Complete. Favourite repositories (up to 30) are stored in SQLite. Polling uses
`If-None-Match`/ETag for repository events and failed Actions runs, establishes a
baseline on the first poll, and keeps up to 160 history rows. Eight categories can be
filtered (commits, pull requests, issues, releases, reviews, workflow failures, stars,
forks). Alerts feed the local notification centre; native notifications through
`tauri-plugin-notification` are sent only when the window is hidden. Background
monitoring is opt-in, throttled to 180 seconds and participates in the 90-second
sync loop. Polling never writes to GitHub.

The repository radar adds three more sources. Starred repositories are fetched live
from `/user/starred`; visit counts and last-visited times are stored locally in
`repo_visits` whenever a repository workspace opens; recommendations search public
GitHub for the top topics and languages represented in stars, favourites and visits,
excluding repositories already in those sets. Recommendation failures are reported
per source and never hide the rest of the radar.

---

## Phase 3 — Advanced GitHub

Pull requests, reviews and inline comments, issues, branch graph, side-by-side diff,
repository search inside Repositories, a standalone Explore/Collections surface with
saved searches, Profile Studio with Markdown preview and a choice between an app-only
draft and publishing to the GitHub profile README repository. Publishing is always
explicit; the draft never leaves the machine
unless the user chooses to sync it.

**Phase 3 complete.** Explore is a standalone surface with keyword, language, topic
and author qualifiers, sorting, quick topic chips, and saved searches in SQLite.
Collections stores multiple local libraries with statuses and notes. Pull requests
have unified and side-by-side diffs, inline comment targets on either side, review
history, and confirmed approve/request-changes/comment actions. Commits can be viewed
as a list or a lane-based branch graph. Profile Studio also sets or clears the live
GitHub status through a confirmed GraphQL mutation; because GitHub requires the
`user` scope, it can start a device-flow reauthorisation. App focus, a 90-second
timer and the TopBar button refresh active GitHub data, while unread notifications
are imported without marking them read online.

**Profile Studio shipped as a builder.** The view fetches a live snapshot
(`github_profile_snapshot`: identity, pins, top repositories, stats, contribution
calendar, socials, public orgs), stores the design in `profile.design` and renders
GitHub Markdown in Rust (`profile_render`). It offers five presets, four layouts,
section reordering, avatar/tagline/bio controls, pins, stats, activity heatmap or
ASCII map, achievement badges (selected manually because GitHub exposes no
achievements API), organisations, custom Markdown and badge styling. The generated
README can be saved into the local `profile.draft`, edited by hand and diffed against
the live README. The only profile write is `github_publish_profile`, behind a
confirmation; `github_create_profile_repo` is a separate confirmed action. Review
actions, inline comments and the branch graph now ship inside the repository
workspace, with Code Review as the repository chooser.

---

## Phase 4 — Extract Studio (complete for copy, move and history split)

`extract.rs` scans a repository without following symlinks or descending into VCS
metadata, dependency caches and build output. It records language statistics,
manifests and the Git branch, then parses imports for Rust, TypeScript/JavaScript,
Svelte, Python, Go, C/C++ and shell into an internal reference graph. Analysis
reports internal edges, incoming and outgoing coupling, external packages, cohesion
and file sizes; suggestions score directories by cohesion, size and manifest
presence. Every plan re-validates the selection and target, refuses traversal and
nested or non-empty destinations, and lists its checks, warnings, writes, backup and
history prefix before anything happens.

Three strategies ship: **copy** writes a verified duplicate and leaves the source
untouched; **move** copies, verifies, writes a full backup under the app data
directory, then removes the originals; **history** clones the repository and uses
`git subtree split --prefix` so the component directory's commits become an
independent repository. Destructive strategies require a second confirmation. The
Rust side rebuilds and re-checks the plan inside execute, and successful runs are
recorded in `extraction_runs`.

Still to add: `git filter-repo` as an alternative history engine, fork/branch
strategies and the cross-project relationship graph. Extract Studio therefore links
completed extractions through its run log rather than a graph view.

---

## Phase 5 — Personal AI

Provider adapters (Anthropic, OpenAI, OpenRouter, Ollama, LM Studio, OpenAI-compatible),
keys in the keyring, explicit context selection, streaming responses, extraction and
review assistants. Every AI action is reviewable and permissioned.

---

## Phase 6 — Marketplace

Plugin manifest spec, capability permissions, signed plugin verification, registry
and installer, theme marketplace, external integrations (Ollama, Docker, `gh`,
rust-analyzer, Claude Code, OpenCode).

---

## Phase 7 — Customization

Dockable/detachable panels, saved workspace layouts, per-project themes and
environments, theme import/export, custom shortcuts, deeper Arch integration,
PKGBUILD and AUR packaging.

---

## Open questions (need a decision before the phase that depends on them)

1. **GitHub OAuth app**: resolved for now — the user supplies their own client ID in
   Repositories or Settings. Whether GRAFT ships a shared public client ID is open.
2. **Keyring crate**: resolved — `keyring` 3 with `sync-secret-service` and
   `crypto-rust`.
3. **Terminal renderer**: `xterm.js` (battle-tested, bigger) vs a slim custom renderer.
   Default: xterm.js unless bundle size becomes a problem.
4. **Git engine**: CLI-only long term, or `gix` for reads with CLI for writes?
   Default: CLI until a measurement says otherwise.
5. **Distribution name / identifier**: currently `GRAFT` + `dev.graft.desktop`.
   Changing the identifier later means a data-directory move, so confirm before v1.
