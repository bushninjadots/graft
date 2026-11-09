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
│  github.rs     auth, repos, search, reviews, issues, releases        │
│  ai.rs         provider adapters, streaming chat, context, history  │
│  relations.rs  local project graph from manifests + run log        │
│  profile.rs    live profile snapshot + README renderer        │
│  watchtower.rs favourites, polling, visits, radar discovery   │
│  terminal.rs   portable-pty sessions over Tauri channels      │
│  library.rs    saved searches, collections and local notes    │
│  git.rs        repository inspection + explicit Git writes     │
│  secrets.rs    keyring token custody                          │
│  extract.rs    repository scanning + extraction plans         │
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
| `src/views/` | Home, Projects, Repositories, Repository, Terminal, Review, Explore, Collections, Extract, ProfileStudio, Watchtower, Settings, AI, ModuleView |
| `src/lib/state/` | `appearance`, `nav`, `dashboard`, `github`, `notifications`, `sync`, `session`, `terminal`, `watchtower`, `library`, `extract`, `ai`, `ui` |
| `src-tauri/src/schema/` | numbered SQL migrations, embedded with `include_str!` |
| `~/.local/share/dev.graft.desktop/graft.db` | local database (WAL) |
| `~/.local/share/dev.graft.desktop/` | app data root (Tauri `app_data_dir`) |

## Data model (schema 6)

```sql
settings(key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)
projects(id INTEGER PK, name TEXT, path TEXT, description TEXT,
         created_at INTEGER, updated_at INTEGER)
watch_repos(id INTEGER PK, owner TEXT, name TEXT, full_name TEXT UNIQUE, ...)
watch_events(id INTEGER PK, repo_id INTEGER, category TEXT, title TEXT, ...)
saved_searches(id INTEGER PK, name TEXT, query TEXT, language TEXT, ...)
collections(id INTEGER PK, name TEXT, description TEXT, ...)
collection_repos(id INTEGER PK, collection_id INTEGER REFERENCES collections(id),
                 full_name TEXT, status TEXT, notes TEXT, ...)
repo_visits(full_name TEXT PK, owner TEXT, name TEXT, language TEXT, topics TEXT,
            visit_count INTEGER, last_visited_at INTEGER, ...)
extraction_runs(id INTEGER PK, source TEXT, target TEXT, name TEXT, strategy TEXT,
                file_count INTEGER, total_bytes INTEGER, backup_path TEXT, status TEXT,
                message TEXT, created_at INTEGER)
ai_conversations(id INTEGER PK AUTOINCREMENT, title TEXT, provider TEXT, model TEXT,
                 endpoint TEXT, created_at INTEGER, updated_at INTEGER)
ai_messages(id INTEGER PK AUTOINCREMENT,
            conversation_id INTEGER REFERENCES ai_conversations(id) ON DELETE CASCADE,
            role TEXT, content TEXT, context TEXT DEFAULT '[]', created_at INTEGER)
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
| `app.notifications.seen` | imported GitHub notification keys, so they are added once |
| `repos.view` | repository layout (`list`, `cards`, `compact`) |
| `repos.scope` | last repository source (`mine`, `explore`) |
| `ai.preferences` | `{ providerId, model, endpoint, system }` — never API keys |

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
| `github_rate_limit` | — | `RateLimits` (core, search and GraphQL windows) |
| `github_set_client_id` | `clientId` | `void` |
| `github_start_device` | — | `DeviceStart` (code, URI, interval, expiry) |
| `github_poll_device` | `deviceCode` | `DevicePoll` — `pending`, `slowDown`, `complete`, `failed` |
| `github_save_token` | `token` | `GithubUser` (verified against `/user` before storage) |
| `github_sign_out` | — | `bool` (credential removed) |
| `github_list_repos` | `page` | `RepoPage` (100 per page, next-page detection) |
| `github_search_repos` | `query, language, topic, author, sort, page` | `RepoSearchPage` (public GitHub search) |
| `github_notifications` | — | unread `GithubNotification[]`; read-only, never marks GitHub read |
| `github_touch_profile` | — | `void` |
| `github_repo_detail` | `owner, repo` | `RepoDetail` plus `starred` |
| `github_repo_readme` | `owner, repo` | `RepoFile \| null` |
| `github_repo_contents` | `owner, repo, path, gitRef` | `RepoEntry[]` |
| `github_repo_file` | `owner, repo, path, gitRef` | `RepoFile` (text, truncated or binary) |
| `github_repo_branches` | `owner, repo` | `RepoBranch[]` |
| `github_repo_commits` | `owner, repo, gitRef, page` | `RepoCommit[]` (40 per page) |
| `github_repo_commit` | `owner, repo, sha` | `RepoCommitDetail` with per-file patches |
| `github_repo_pulls` | `owner, repo, state` | `RepoPull[]` (`open`, `closed`, `all`) |
| `github_repo_pull` | `owner, repo, number` | `RepoPullDetail` with files and `headSha` |
| `github_repo_pull_reviews` | `owner, repo, number` | `RepoPullReview[]` |
| `github_repo_pull_comments` | `owner, repo, number` | `RepoPullComment[]` |
| `github_submit_pull_review` | `owner, repo, number, event, body` | `RepoPullReview` — confirmed external write |
| `github_create_pull_comment` | `owner, repo, number, body, commitId, path, line, side` | `RepoPullComment` — confirmed external write |
| `github_repo_issues` | `owner, repo, state` | `RepoIssue[]` (pull requests excluded) |
| `github_repo_releases` | `owner, repo` | `RepoRelease[]` (30 most recent, drafts included) |
| `github_create_issue` | `owner, repo, title, body` | `RepoIssue` |
| `github_fork` | `owner, repo` | `ForkResult` |
| `github_set_star` | `owner, repo, starred` | `bool` |
| `github_profile_readme` | `login` | `ProfileReadme` (`exists`, content, sha, htmlUrl) |
| `github_profile_snapshot` | — | live pins, stats, contribution calendar, socials, public orgs |
| `github_set_profile_status` | `emoji, message` | `ProfileStatus \| null` — live change, requires the `user` scope; empty values clear |
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
| `watchtower_snapshot` | — | `WatchtowerSnapshot` (repos, events, unread) |
| `watchtower_watch` | `owner, repo` | `WatchtowerRepo` |
| `watchtower_unwatch` | `id` | `void` |
| `watchtower_poll` | `categories` | `WatchtowerPoll` (conditional requests, new events, errors) |
| `watchtower_mark_read` | `id \| null` | unread count |
| `watchtower_discover` | — | `WatchtowerDiscovery` (live stars, local visits, derived recommendations, per-source errors) |
| `watchtower_record_visit` | `repo` metadata | `void` — local upsert; increments visit count |
| `terminal_environment` | — | available shells and default cwd |
| `terminal_start` | `cwd, shell, cols, rows, onEvent` | `TerminalSessionInfo` over a Tauri channel |
| `terminal_write` / `terminal_resize` / `terminal_close` | session args | `void` |
| `explore_saved_searches` | — | `SavedSearch[]` |
| `explore_save_search` | `name, query, language, topic, author, sort` | `SavedSearch` |
| `explore_delete_search` | `id` | `void` |
| `collections_list` / `collections_create` / `collections_delete` | collection args | `Collection[]` / `Collection` / `void` |
| `collection_detail` | `id` | `CollectionDetail` |
| `collection_add_repo` | `collectionId, owner, repo` | `CollectionRepo` (metadata read from GitHub) |
| `collection_update_repo` | `id, status, notes` | `CollectionRepo` |
| `collection_remove_repo` | `id` | `void` |
| `extract_scan` | `source` | `ExtractScan` (read-only, skips symlinks and caches) |
| `extract_suggest` | `source` | `ExtractComponent[]` scored directories |
| `extract_analyze` | `source, selection` | `ExtractAnalysis` (internal, incoming, outgoing, external) |
| `extract_plan` | `source, selection, strategy, target, name, engine?` | `ExtractPlan` with checks, warnings and actions |
| `extract_execute` | `source, selection, strategy, target, name, engine?, confirmed` | `ExtractResult`; re-validates and records the run |
| `extract_runs` | — | `ExtractionRun[]` (recent successful runs) |
| `project_graph` | — | `ProjectGraph` (projects, manifests, shared remotes, dependencies, extraction edges) |
| `ai_catalog` | — | `AiCatalog` (providers plus keyring status per provider) |
| `ai_save_key` | `provider, key` | `void` — writes only the Secret Service entry |
| `ai_clear_key` | `provider` | `bool` (credential removed) |
| `ai_conversations` | — | `AiConversation[]` |
| `ai_conversation` | `id` | `AiConversationDetail` with messages and context titles |
| `ai_delete_conversation` | `id` | `void` — cascades to messages |
| `ai_context_scan` | `projectId` | `AiContextFile[]` (read-only local listing) |
| `ai_context_read` | `projectId, paths` | `AiContextItem[]` (path-confined, binary-safe, truncated) |
| `ai_cancel` | `requestId` | `void` — stops the active stream after the current chunk |
| `ai_chat` | `request, channel` | `AiReply`; streams `started`, `delta`, `done` or `cancelled` events |

Serde maps snake_case fields to camelCase for JS. GitHub payloads deserialize from
the API's snake_case and serialize with `rename_all(serialize = "camelCase")`. Bump this table when adding a
command, and add the matching wrapper in `lib/api.ts`.

## Security posture

- CSP permits own assets, `ipc:` and images from GitHub's avatar, raw and camo
  hosts. No API origin is reachable from the webview; GitHub and AI traffic are
  Rust-only.
- Markdown (README, pull request bodies) is rendered with `marked` and sanitized
  with DOMPurify; links are handed to `open_external`, never navigated in-app.
- File contents come from the contents API, are decoded in Rust, and are refused
  when binary or larger than 512 KB.
- Capability file grants `core:default` only. No shell, fs or http plugin.
- The UI has no direct filesystem, process or network access.
- GitHub tokens live in the Secret Service through `keyring`; SQLite stores only the
  public OAuth client ID and the cached profile.
- Tokens never cross IPC: `github_status` reports presence and identity, not the token.
- AI provider keys use the same Secret Service store under `ai:<provider>`; the
  catalog reports only whether a key is configured. Required keys are re-checked in
  Rust before a request, and Google credentials travel in the `x-goog-api-key` header,
  never in a URL. Remote endpoints require HTTPS; plain HTTP is accepted only for
  loopback hosts. Endpoints cannot embed credentials, query strings or fragments,
  and redirects are not followed.
- AI context is opt-in. The workspace lists local project files read-only, and file
  contents are returned only for the paths the user attached. Any endpoint whose host
  is not loopback requires an explicit consent checkbox before selected text leaves
  the machine; Rust re-checks that consent before saving or sending the request.
  Context titles and conversation messages stay in local SQLite, while attached file
  contents are not persisted.
- AI responses are sanitized Markdown and never execute tools or commands. Chat
  requests are cancelled in Rust, and a partial reply is still saved when stopped.
  Review and extraction assistants only prefill a conversation; they never submit a
  review, change a file or run an extraction.
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
  navigate inside GRAFT and never performs a write. GitHub's unread notifications are
  imported read-only and are never marked read on github.com. Watchtower stores only
  favourite slugs, alert metadata and local visit counts; polling and discovery are
  read-only. Starred repositories come from `/user/starred`; GRAFT never stars or
  unstars a repository for the radar. Similar repositories are public GitHub search
  results, clearly derived from local interests rather than presented as curated data.
- Review approvals, change requests and inline comments are separate external writes
  with an explicit confirmation step. The Rust side validates the review event, file
  path, head commit, diff side and line before calling GitHub.
- Terminal sessions are owned by Rust. The shell path and working directory are
  validated, output is streamed over a per-session channel, and closing a tab kills
  its child process. The webview never spawns a process directly.
- Status changes are a separate confirmed external write. The token must have the
  `user` scope; the Profile Studio can start a device-flow reauthorisation for it.
- Background sync only reads. It refreshes the active GitHub surface on focus and a
  90-second timer, and polls unread notifications; it does not push local Git commits.
- Destructive operations (delete project, git reset, extraction) must be confirmed
  in the UI and re-validated in Rust.

## Performance approach

- Lazy: no network, polling or watcher starts on boot; the engine opens SQLite only.
- Lists are plain DOM for now; virtualise at the point a list can exceed ~200 rows.
- `detect_tools` runs on demand and on the Environment module's refresh action.
- GitHub calls stay view-scoped: account refresh and unread notifications every
  90 seconds while signed in, repository pages only while a repository view is active
  or on explicit Refresh, and Git status per linked project on Projects. Focus sync
  is throttled and every loop is silent on failure.
- Large lists remain plain DOM until they exceed roughly 200 rows; pagination keeps
  repository and search responses bounded.
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
