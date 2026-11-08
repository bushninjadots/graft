# GRAFT — agent notes

Read `docs/PLAN.md` (scope/status), `docs/ARCHITECTURE.md` (boundaries) and
`docs/DESIGN.md` (visual contract) before changing code.

## Commands

```bash
npm run app:dev     # desktop dev window
npm run check       # svelte-check (types + template diagnostics)
npm run build       # vite production build
npm run check:rust  # cargo check
npm run verify      # all of the above
cargo test --manifest-path src-tauri/Cargo.toml   # unit tests
```

`npm run dev` alone serves the UI in a browser; the backend is absent there by
design and the UI shows an offline state.

## Non-negotiables

- Tauri 2 + Rust engine, Svelte 5 runes, TypeScript, SQLite. No Electron, no SPA
  pretending to be native, no cloud dependency.
- All filesystem, git, network and process access lives in Rust. The webview calls
  commands through `src/lib/api.ts`.
- GitHub tokens belong in the Secret Service keyring (`secrets.rs`). Never SQLite,
  logs, localStorage or a command response.
- No colour literals in components: use the tokens from `themes.ts` via CSS vars.
- No fabricated data. Unimplemented modules render `ModuleView` status panels.
- Never report success before a backend operation resolves.
- Destructive operations require explicit confirmation, and the Rust side must
  re-validate the request.
- Adding a command means: handler in `commands.rs`, registration in `lib.rs`,
  wrapper in `api.ts`, row in the IPC table in `docs/ARCHITECTURE.md`.
- Schema changes ship as a new numbered SQL file plus a `SCHEMA_VERSION` bump and a
  migration branch. Never edit a shipped migration.

## Svelte 5 conventions

- Runes: `$state`, `$derived`, `$effect`, `$props`. Event attributes (`onclick`),
  not `on:click`.
- Shared reactive state lives in `src/lib/state/*.svelte.ts` and is exported as a
  single `$state` object plus functions.
- Keep persistence in the state module (debounced), not in components.

## Before claiming a change is done

Run `npm run check` and, for Rust changes, `npm run check:rust`. Report failures
rather than working around them.
