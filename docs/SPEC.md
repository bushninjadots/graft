# GRAFT — product brief

Binding requirements distilled from the master blueprint. Where this file and the
blueprint disagree, the blueprint wins and this file is corrected.

## Vision

A lightweight, highly customisable desktop application for Arch Linux that acts as a
personal GitHub command centre: account and repository management, discovery,
monitoring, Git operations, project workspaces, a real terminal, code review,
provider-agnostic AI, repository intelligence, component extraction into independent
projects, profile customisation, plugins, themes and Linux integration.

GRAFT is a personal development environment built around GitHub — not a replacement
for GitHub. It must be useful without AI, subscriptions, cloud services or an account
beyond GitHub itself.

Five capabilities define the product: **Control, Intelligence, Creation,
Personalization, Freedom.**

## Non-negotiable technical direction

- Rust engine, Tauri 2 shell, Svelte 5 + TypeScript interface, CSS token design system.
- SQLite for local state, migrations included. GitHub is authoritative for remote data.
- Git through the installed Git CLI. GitHub through REST + GraphQL, never scraping.
- Tokens in the OS keyring, never in SQLite, config files or webview storage.
- native PTY terminal.
- Arch Linux first; Hyprland and Wayland first-class, X11 where practical.
- Native notifications, secrets service, XDG paths, SSH/GPG, desktop entry, PKGBUILD.

## Modules

Dashboard · My Projects · Repositories · Watchtower · Git Workspace · Code Review ·
Extract Studio · Explore · Collections · AI Workspace · Profile Studio · Terminal ·
Plugin Marketplace · Settings. The sidebar is user-controlled: reorder, hide, pin,
group, compact/expanded.

## Signature features

1. **Watchtower** — favourite any repository, per-event monitoring (commits, releases,
   tags, PRs, issues, discussions, Actions, advisories, dependencies, visibility,
   archival), severity categories, read/mute/snooze, and a dedicated alert screen.
   Event-driven where possible, conditional requests, no excessive polling, opt-in
   background worker.
2. **Extract Studio** — analyse a repository, identify an independent component, show
   external dependencies, internal references and coupling, then extract with copy,
   move, extract-and-replace, split-history, fork or branch strategies. Validate before
   writing, preserve history deliberately (never silently discard), produce a
   reviewable plan, and link the result in a project relationship graph.
3. **Repository intelligence** — explain what changed since the last visit by files,
   directories and logical components; AI summaries optional.
4. **Profile Studio** — visual README authoring with live preview, diff against the
   live GitHub README, deliberate sync, GitHub-compatible Markdown only.
5. **Personal AI** — Anthropic, OpenAI, Google, OpenRouter, Ollama, LM Studio,
   llama.cpp and OpenAI-compatible endpoints; selective context; explicit permission
   before private content leaves the machine; AI is never mandatory.

## Design law

Modern industrial developer interface: precise alignment, thin borders, exceptional
typography, compact density, restrained accent colour, subtle shadows, fast deliberate
animation, no oversized rounded cards, no glassmorphism, no decorative gradients.
Themes control every colour; layouts, density and panels are adjustable per user and
per project.

## Security and safety law

Secure credential storage, explicit filesystem permissions, terminal execution
approval, AI data-sharing controls, capability-based plugin isolation, input
validation, no silent destructive Git operations, backup before destructive
extraction, confirmation before publishing, no privileged commands without approval,
and an audit trail for important actions.

## Performance law

Fast startup, low idle CPU and memory, no continuous rescanning, no unnecessary
polling, lazy modules, virtualised large lists, incremental scans, cached responses,
cancellable background work. Benchmarks are measured, not promised.

## What not to build

No Electron. No browser site in a desktop window. No Copilot requirement. No
subscription or hosted service requirement. No mandatory AI. No microservice sprawl.
No duplicated GitHub website. No scraping. No unnecessary background processes. No
visually heavy interface. No unsandboxed plugin execution. No silent repository
rewrites. No automatic publishing of private code. No sacrificing function for
animation.

## Long-term

Multiple accounts and hosts (GitLab, Codeberg, Gitea/Forgejo), dependency graphs,
component libraries, advanced worktrees, automation, transparent health scoring, plugin
ecosystem, project templates, environment profiles, multi-repository workflows and
advanced agents — without architectural rewrites.
