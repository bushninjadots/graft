# GRAFT

**Build. Connect. Extract. Evolve.**

GRAFT is a personal GitHub environment for the Linux desktop: repositories, projects,
review, monitoring, extraction and developer tooling in one lightweight application.
It is a native desktop instrument (Tauri 2 + Rust + Svelte 5), not a website in a
window.

> Status: **Phase 1 — GitHub**. Real OAuth device-flow sign-in with the token kept in
> the system keyring, a live repository list, and detected Git state for local
> projects. Terminal, Git write operations, code review, Watchtower, Extract Studio,
> AI and plugins remain scheduled phases and render explicit planned modules — never
> fake data. See [`docs/PLAN.md`](docs/PLAN.md).

## What works today

- Native window with the GRAFT shell (sidebar, top bar, status bar).
- GitHub authentication through the OAuth **device flow** — or a personal access
  token — with the token written only to the Secret Service keyring.
- **Repositories**: public and private repositories from the GitHub REST API, with
  filter, pagination, language, stars, visibility and archived state. Links open in
  your browser; GRAFT never embeds a GitHub webpage.
- **My Projects**: create and remove project records, validate directories in Rust,
  and read real Git state for each linked directory — branch, change count,
  ahead/behind, remote and GitHub slug. A Detect action fills the name and previews
  the repository before saving.
- 7 themes with instant switching, three density levels, adjustable corner radius.
- Navigation with groups, pinning, hiding, reordering; all persisted in SQLite.
- Dashboard with draggable, hideable modules and a persisted layout.
- Command palette (`Ctrl K`) for navigation, themes, density and layout actions.
- Environment panel: detects installed developer tools from `PATH` with versions.
- Local settings, schema versioning and an offline-visible failure mode.

## Stack

| Layer | Choice |
| ----- | ------ |
| Shell | Tauri 2 |
| Engine | Rust 2021 — rusqlite (bundled SQLite), reqwest, keyring, serde |
| UI | Svelte 5 runes + TypeScript |
| Build | Vite 7, npm |
| Design | CSS token system driven by the theme engine |

## Requirements (Arch / EndeavourOS)

```bash
sudo pacman -S --needed base-devel rust nodejs npm \
  webkit2gtk-4.1 gtk3 libsoup3 librsvg imagemagick
```

For GitHub sign-in, a Secret Service provider must be running — `gnome-keyring` or
KWallet. Without one GRAFT reports that the keyring is unavailable instead of
falling back to a plaintext token.

Optional tooling GRAFT reports on if present: `git`, `github-cli`, `python`, `go`,
`docker`, `podman`, `openssh`, `gnupg`, `fish`, `zsh`, Hyprland (`hyprctl`).

## Run

```bash
npm install
npm run app:dev        # desktop window (Vite dev server on :1420)
```

Production build:

```bash
npm run app:build      # release binary + deb/appimage bundles in src-tauri/target/release/bundle
```

Verification:

```bash
npm run check          # svelte-check: TypeScript + template diagnostics
npm run build          # production frontend bundle
npm run check:rust     # cargo check
npm run verify         # all three
cargo test --manifest-path src-tauri/Cargo.toml   # 7 unit tests
```

`npm run dev` alone serves the UI in a browser. The Rust backend is absent there by
design; the interface shows a clear offline state instead of pretending to work.

## Connecting GitHub

1. Create a GitHub OAuth app under **Settings → Developer settings → OAuth Apps** and
   enable **Device Flow**. No client secret is used or stored.
2. In **Repositories**, paste the app's client ID and press **Save**.
3. Press **Connect GitHub**, then enter the displayed code at
   `github.com/login/device`.

The device flow requests `repo` and `read:user`. A personal access token with the
same scopes is accepted as an alternative and verified against `api.github.com`
before it is stored.

## Layout

```
src/                  Svelte 5 interface
  views/              Home, Projects, Repositories, Settings, ModuleView
  lib/components/     shell, palette and dashboard panels
  lib/state/          reactive state (appearance, nav, dashboard, github, session, ui)
  lib/api.ts          the only IPC surface
src-tauri/src/        Rust engine (commands, db, schema, github, git, secrets, tools)
  src-tauri/src/schema/  versioned SQL migrations
docs/                 plan, architecture, design, product brief
scripts/              icon rendering
```

## Storage and security

- Database: `~/.local/share/dev.graft.desktop/graft.db` (SQLite, WAL).
- Secrets: GitHub tokens live in the system keyring (`dev.graft.desktop` service).
  They are never written to SQLite, logs, localStorage or the webview.
- Network: all GitHub requests are made by Rust. The webview CSP allows no external
  connections and only avatar images from `avatars.githubusercontent.com`.
- External links: `open_external` accepts `http(s)` only.
- Local Git access is read-only inspection of directories you explicitly link.

## Documentation

- [`docs/PLAN.md`](docs/PLAN.md) — phases, decisions, open questions.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — boundaries, IPC, schema, security.
- [`docs/DESIGN.md`](docs/DESIGN.md) — visual identity and token contract.
- [`docs/SPEC.md`](docs/SPEC.md) — binding product requirements.
- [`AGENTS.md`](AGENTS.md) — working rules for contributors and coding agents.
