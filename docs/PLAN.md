# GRAFT — build plan

Living document. Update it when scope, decisions or phase status change.

Product spec: `docs/SPEC.md` (the blueprint this plan implements).
Architecture: `docs/ARCHITECTURE.md`. Visual system: `docs/DESIGN.md`.

---

## Current status

**Phase 1 — GitHub: auth, repository workspace and local Git actions complete.**
Phase 1a added auth, the repository list and local Git inspection. Phase 1b added the
in-app repository workspace (README, code tree, commits with diffs, pull requests,
branches), star/fork/clone, and fetch/pull/push for linked projects. Phase 0
(foundation) is complete and verified as well.

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
| 13 | Terminal, commit and branch write operations | next (Phase 1b) |

Still planned: terminal, commit/branch write operations (with confirmation), ETag
caching, rate-limit surfacing beyond error messages, issues and releases.

### Verification (Phase 1a)

| Check | Result |
| ----- | ------ |
| `svelte-check` | 0 errors, 0 warnings |
| `vite build` | passes — 248 kB JS / 45 kB CSS (marked + DOMPurify included) |
| `cargo check` | passes |
| `cargo test` | 11/11 pass (migrations, settings, projects, Git parsing, branch validation, a real commit/branch cycle in a temp repo, device flow, HTTP client, profile publish payload) |
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

## Phase 1 — Core GRAFT (next)

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

Favourite repos, event polling with conditional requests, GitHub notifications,
native notifications via `tauri-plugin-notification`, alert categories, history in
SQLite, optional background worker behind a setting (default off).

---

## Phase 3 — Advanced GitHub

Pull requests, reviews and inline comments, issues, branch graph, side-by-side diff,
Explore/Collections with saved searches, Profile Studio with Markdown preview and a
choice between an app-only draft and publishing to the GitHub profile README
repository. Publishing is always explicit; the draft never leaves the machine
unless the user chooses to sync it.

**Profile Studio shipped first.** The view stores its draft in the SQLite settings
blob (`profile.draft`, `profile.draftSavedAt`), debounces autosave, renders a
sanitized preview, and diffs the draft against the live README fetched by Rust. The
only write is `github_publish_profile`, behind a confirmation dialog showing the
target repository and commit message. Review actions, issues and the branch graph
remain open in this phase.

---

## Phase 4 — Extract Studio

Language adapters (Rust, TS/JS, Python, Go, C/C++, shell) over a repository scanner;
dependency and internal-reference analysis; extraction plans; copy/move/split/fork;
`git filter-repo` for history; validation report; project relationship graph.

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
