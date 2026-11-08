# GRAFT — architecture

## Shape

```
┌──────────────────────────────────────────────────────────────┐
│ Svelte 5 UI (src/)                                           │
│  views/            one screen per sidebar module              │
│  lib/components/   shell: sidebar, topbar, palette, statusbar │
│  lib/state/*.svelte.ts   reactive state + persistence         │
│  lib/themes.ts     theme token contract                       │
│  lib/api.ts        typed IPC surface (the only invoke() site) │
└───────────────────────────┬──────────────────────────────────┘
                            │ tauri IPC (JSON, camelCase)
┌───────────────────────────▼──────────────────────────────────┐
│ Rust engine (src-tauri/src/)                                  │
│  commands.rs   thin command handlers, validation, errors      │
│  db.rs         SQLite, migrations, queries                    │
│  tools.rs      environment probing                            │
│  github.rs     device flow, API client, repository + issues    │
│  profile.rs    live profile snapshot + README renderer        │
│  git.rs        repository inspection + explicit Git writes     │
│  secrets.rs    keyring token custody                          │
│  extract/      (Phase 4) repository analysis                  │
└───────────────────────────┬──────────────────────────────────┘
                            │
              SQLite (XDG data dir) · keyring (secrets) · OS tools
```

Rules that keep this maintainable:

- `lib/api.ts` is the only module that calls `invoke`. Views never build IPC strings.
- Commands validate input and return `Result<T, String>`; the UI shows the message.
- The Rust side owns all filesystem, git and network access. The webview is not
  trusted with paths, tokens or subprocesses.
- No feature reaches across another module's files. Shared code moves to `lib/`.

## Runtime layout

| Path | Purpose |
| ---- | ------- |
| `src/App.svelte` | shell composition, keyboard shortcuts, boot sequence |
| `src/views/` | Home, Projects, Repositories, Repository, ProfileStudio, Settings, ModuleView |
| `src/lib/state/` | `appearance`, `nav`, `dashboard`, `github`, `session`, `ui` |
| `src-tauri/src/schema/` | numbered SQL migrations, embedded with `include_str!` |
| `~/.local/share/dev.graft.desktop/graft.db` | local database (WAL) |
| `~/.local/share/dev.graft.desktop/` | app data root (Tauri `app_data_dir`) |

## Data model (schema 1)

```sql
settings(key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)
projects(id INTEGER PK, name TEXT, path TEXT, description TEXT,
         created_at INTEGER, updated_at INTEGER)
```

Migrations are append-only: add `schema/00N_*.sql`, bump `SCHEMA_VERSION`, add an
`if current < N` branch in `db::migrate`. Never edit a shipped migration.

Settings blobs currently stored:

| Key | Shape |
| --- | ----- |
| `appearance` | `{ themeId, density, radius, sidebarCollapsed }` |
| `nav.preferences` | `{ order: string[], hidden: string[], pinned: string[] }` |
| `dashboard.layout` | `{ order: string[], hidden: string[] }` |
| `github.clientId` | public OAuth app client ID (not a secret) |
| `github.profile` | cached `GithubUser` JSON for offline display |
| `profile.draft` | Profile Studio Markdown, never uploaded implicitly |
| `profile.draftSavedAt` | last local draft save (epoch seconds) |
| `profile.design` | visual builder design JSON |
| `profile.designSavedAt` | last design save (epoch seconds) |
| `app.notifications` | local notification centre (max 80, unread state) |
| `repos.view` | repository layout (`list`, `cards`, `compact`) |

Settings writes are debounced in the UI (200 ms) and upserted with `ON CONFLICT`.

## IPC contract

| Command | Args | Returns |
| ------- | ---- | ------- |
| `app_info` | — | `{ version, dataDir, dbPath, schemaVersion }` |
| `get_setting` | `key` | `string \| null` |
| `set_setting` | `key, value` | `void` |
| `list_projects` | — | `Project[]` |
| `create_project` | `name, path, description` | `Project` |
| `delete_project` | `id` | `void` |
| `detect_tools` | — | `ToolInfo[]` |
| `project_git_status` | `path` | `GitStatus` (never fails; errors are reported in the payload) |
| `open_external` | `url` | `void` — rejects non-http(s) schemes |
| `github_status` | — | `AuthStatus` (keyring probe, profile refresh, scopes) |
| `github_set_client_id` | `clientId` | `void` |
| `github_start_device` | — | `DeviceStart` (code, URI, interval, expiry) |
| `github_poll_device` | `deviceCode` | `DevicePoll` — `pending`, `slowDown`, `complete`, `failed` |
| `github_save_token` | `token` | `GithubUser` (verified against `/user` before storage) |
| `github_sign_out` | — | `bool` (credential removed) |
| `github_list_repos` | `page` | `RepoPage` (100 per page, next-page detection) |
| `github_touch_profile` | — | `void` |
| `github_repo_detail` | `owner, repo` | `RepoDetail` plus `starred` |
| `github_repo_readme` | `owner, repo` | `RepoFile \| null` |
| `github_repo_contents` | `owner, repo, path, gitRef` | `RepoEntry[]` |
| `github_repo_file` | `owner, repo, path, gitRef` | `RepoFile` (text, truncated or binary) |
| `github_repo_branches` | `owner, repo` | `RepoBranch[]` |
| `github_repo_commits` | `owner, repo, gitRef, page` | `RepoCommit[]` (40 per page) |
| `github_repo_commit` | `owner, repo, sha` | `RepoCommitDetail` with per-file patches |
| `github_repo_pulls` | `owner, repo, state` | `RepoPull[]` (`open`, `closed`, `all`) |
| `github_repo_pull` | `owner, repo, number` | `RepoPullDetail` with files |
| `github_repo_issues` | `owner, repo, state` | `RepoIssue[]` (pull requests excluded) |
| `github_create_issue` | `owner, repo, title, body` | `RepoIssue` |
| `github_fork` | `owner, repo` | `ForkResult` |
| `github_set_star` | `owner, repo, starred` | `bool` |
| `github_profile_readme` | `login` | `ProfileReadme` (`exists`, content, sha, htmlUrl) |
| `github_profile_snapshot` | — | live pins, stats, contribution calendar, socials, public orgs |
| `profile_render` | `design, snapshot` | GitHub-flavoured Markdown (pure renderer) |
| `github_publish_profile` | `login, markdown, sha, message` | `PublishResult` — the only profile README upload path |
| `github_create_profile_repo` | `login` | `ProfileRepoResult` — explicit public repo creation |
| `github_save_file` | `owner, repo, path, content, sha, branch, message` | `FileWriteResult` (push permission required) |
| `clone_destination` | `owner, repo` | suggested `~/Projects/<owner>-<repo>` path |
| `clone_repo` | `url, dest` | path (refuses an existing destination) |
| `git_action` | `path, action` | `GitActionResult` (`fetch`, `pull --ff-only`, `push` only) |
| `git_branches` | `path` | `GitBranch[]` with current/upstream flags |
| `git_stage` | `path` | `GitActionResult` (`git add -A`) |
| `git_commit` | `path, message, stageAll` | `GitActionResult` (optionally stages first) |
| `git_create_branch` | `path, name, switch` | `GitActionResult` |
| `git_switch_branch` | `path, name` | `GitActionResult` |

Serde maps snake_case fields to camelCase for JS. GitHub payloads deserialize from
the API's snake_case and serialize with `rename_all(serialize = "camelCase")`. Bump this table when adding a
command, and add the matching wrapper in `lib/api.ts`.

## Security posture (Phase 1)

- CSP permits own assets, `ipc:` and images from GitHub's avatar, raw and camo
  hosts. No API origin is reachable from the webview; GitHub traffic is Rust-only.
- Markdown (README, pull request bodies) is rendered with `marked` and sanitized
  with DOMPurify; links are handed to `open_external`, never navigated in-app.
- File contents come from the contents API, are decoded in Rust, and are refused
  when binary or larger than 512 KB.
- Capability file grants `core:default` only. No shell, fs or http plugin.
- The UI has no direct filesystem, process or network access.
- GitHub tokens live in the Secret Service through `keyring`; SQLite stores only the
  public OAuth client ID and the cached profile.
- Tokens never cross IPC: `github_status` reports presence and identity, not the token.
- `open_external` accepts `http(s)` only. Local Git writes are limited to stage,
  commit, branch create/switch, fetch, pull and push on directories the user linked;
  branch names are validated before they reach `git`.
- Profile publishing is a single explicit command behind a confirmation dialog. The
  draft is stored locally and is never uploaded by any read path. Creating the
  profile repository is a separate confirmed command.
- Repository file editing is offered only when `permissions.push` is true in the repo
  payload, and commits go through a second confirmation with the branch, path and
  message. GitHub remains the final authority.
- The notification centre is local state in SQLite settings; opening an item can only
  navigate inside GRAFT and never performs a write.
- Destructive operations (delete project, git reset, extraction) must be confirmed
  in the UI and re-validated in Rust.

## Performance approach

- Lazy: no network, polling or watcher starts on boot; the engine opens SQLite only.
- Lists are plain DOM for now; virtualise at the point a list can exceed ~200 rows.
- `detect_tools` runs on demand and on the Environment module's refresh action.
- GitHub calls happen only when a view needs them: status at boot (one `/user`),
  repositories when Repositories opens, Git status per linked project on Projects,
  and each repository tab when it is first shown. Nothing polls in the background.
- Typesetting and geometry come from CSS tokens; no JS layout measurement in the shell.

## Build and run

```bash
npm install          # frontend deps + Tauri CLI
npm run app:dev      # Tauri dev window (Vite on :1420)
npm run verify       # svelte-check + vite build + cargo check
npm run app:build    # release binary + deb/appimage bundles
```

Arch prerequisites: `webkit2gtk-4.1`, `gtk3`, `libsoup3`, `base-devel`, `rust`,
`nodejs`, `npm`. Details in `README.md`.
