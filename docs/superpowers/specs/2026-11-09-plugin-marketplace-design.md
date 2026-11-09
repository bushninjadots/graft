# GRAFT Phase 6 — Plugin Marketplace design

Status: approved · Date: 2026-11-09 · Phase: 6

## Intent

Phase 6 adds a plugin marketplace without weakening GRAFT's security posture.
The project spec forbids unsandboxed plugin execution, and the Tauri shell
deliberately exposes no filesystem, shell or HTTP capability to the webview.
The marketplace therefore treats a plugin as **signed, declarative data**, not
as code: a strict JSON manifest plus a signature, with two supported kinds —
themes and integration metadata. Anything that needs to run code is out of
scope until a sandboxed runtime exists as its own project.

The result should let a user point GRAFT at a registry they trust, review what
a package can do, install it after an explicit key-trust decision, use a theme
immediately, and see which external tools are present on the machine. It must
never silently execute, fetch, or write anything on a plugin's behalf.

## Goals

1. Define a versioned, strictly validated plugin manifest format.
2. Make capability declarations meaningful: a theme package can apply theme
   tokens and nothing else; an integration package can describe detected tools
   and nothing else.
3. Verify every installed package with SHA-256 and an Ed25519 detached
   signature before it reaches the database.
4. Provide a registry client and an installer with explicit review and trust.
5. Surface installed themes in the existing theme system, including Settings.
6. Surface external integrations (Ollama, Docker, `gh`, rust-analyzer, Claude
   Code, OpenCode) with real detection status from the Rust side.
7. Keep every operation auditable and reversible: list, enable/disable,
   uninstall, view trusted keys, remove a trusted key.

## Non-goals

- Executable plugins, WASM, native shared libraries or spawned processes.
- A hosted GRAFT marketplace or a fabricated built-in catalog.
- Automatic updates, background polling, telemetry or publishing.
- Arbitrary filesystem paths, archive extraction, or plugin-controlled CSS.
- Plugin-defined shell commands, tool invocations or network requests.
- Multi-version installation of the same plugin, channels or rollback beyond
  uninstall/reinstall.
- Windows/macOS-specific signing integration.

## Anti-goals

The phase fails if any of these are true:

- A package can cause code execution, spawn a process, read an arbitrary file
  or make a network request outside the registry/manifest fetch.
- A theme value can inject CSS functions such as `url()`, `var()` or
  `expression()`, or otherwise escape the token contract.
- A new signing key is trusted without the user seeing its fingerprint and
  explicitly agreeing.
- A registry can replace the key for an already-installed plugin id.
- An unsigned, corrupted, mismatched or downgraded package is installed.
- The UI shows invented plugins, install counts or integration status.

## Constraints

From `docs/SPEC.md` and `AGENTS.md`:

- Capability-based plugin isolation; no unsandboxed plugin execution.
- All network and filesystem access lives in Rust; the webview calls commands
  through `src/lib/api.ts`.
- No colour literals in components; themes drive CSS variables.
- Schema changes ship as a new numbered SQL file plus a `SCHEMA_VERSION` bump
  and a migration branch. Schema 6 is current, so this phase adds schema 7.
- Adding a command means a handler, registration in `lib.rs`, an `api.ts`
  wrapper and a row in the IPC table in `docs/ARCHITECTURE.md`.
- Destructive operations require explicit confirmation and Rust-side
  revalidation.
- Svelte 5 runes; shared state in `src/lib/state/*.svelte.ts`; event attributes,
  not `on:click`.

## Approaches considered

### A — Declarative signed packages (chosen)

A plugin is a signed JSON document. Themes and integration metadata are data,
validated in Rust and stored in SQLite. No plugin code runs. Capabilities are
enforced by the schema: the only two capabilities that exist are the two the
app implements, and a package's capability list must match its kind exactly.

- Pros: satisfies the security law honestly, small dependency surface, no
  archive or path-traversal risk, easy audit and uninstall, themes become
  possible immediately.
- Cons: plugins cannot add behaviour. A future executable-plugin effort will
  need a separate sandboxed runtime.

### B — WASM-sandboxed plugins

Add a WASM runtime and a host ABI for commands, panels and selected project
reads. This is the honest long-term path to code plugins, but it is larger than
the rest of Phase 6 combined, and the panel extension points it would need
belong with Phase 7's dockable panels. Deferred as its own project.

### C — Native executables with declared arguments

A manifest names an executable and arguments; GRAFT spawns it under declared
permissions. Simple and powerful, but it is unsandboxed plugin execution and
violates the spec outright. Rejected.

The user chose A. B remains the intended direction for a later phase; C is not
an option.

## Plugin model

### Package format

A package is exactly two documents fetched from URLs:

1. The **manifest**: UTF-8 JSON, at most 64 KiB, byte-for-byte what is signed.
2. The **signature**: base64 text of a 64-byte Ed25519 signature over the exact
   manifest bytes, at most 1 KiB including whitespace.

There is no archive, no directory tree, no executable payload and no path
contained in a package. A registry entry supplies the public key; a local
developer can serve a registry over loopback HTTP.

### Manifest schema (schema 1)

Unknown fields are rejected (`deny_unknown_fields`) so a manifest cannot smuggle
data past the validator. `schema` must be `1`.

```json
{
  "schema": 1,
  "id": "dev.example.night-owl",
  "name": "Night Owl",
  "version": "1.2.0",
  "kind": "theme",
  "summary": "A low-glare theme for late sessions.",
  "author": "Example Author",
  "homepage": "https://example.dev/night-owl",
  "minAppVersion": "0.1.0",
  "capabilities": ["theme.apply"],
  "theme": {
    "scheme": "dark",
    "blurb": "Deep blue · warm amber",
    "tokens": {
      "bg": "#0b0d12",
      "bgElev": "#0f1218",
      "surface": "#121620",
      "surfaceAlt": "#171c28",
      "border": "#232a3a",
      "borderStrong": "#333d52",
      "text": "#e2e6ee",
      "textMuted": "#9aa3b4",
      "textFaint": "#6a7384",
      "accent": "#d9a441",
      "accentSoft": "rgba(217, 164, 65, 0.14)",
      "accentInk": "#15120a",
      "ok": "#79b47a",
      "warn": "#d9a441",
      "danger": "#d16a5f",
      "info": "#7ba7c4",
      "shadow": "rgba(0, 0, 0, 0.45)"
    }
  }
}
```

An integration manifest replaces `theme` with:

```json
{
  "kind": "integration",
  "capabilities": ["integration.inspect"],
  "integration": {
    "tools": ["ollama", "docker"],
    "notes": "Start Ollama before using local models in the AI workspace.",
    "docsUrl": "https://example.dev/notes"
  }
}
```

Fields and validation:

| Field | Rule |
|---|---|
| `schema` | integer, exactly `1` |
| `id` | `^[a-z0-9][a-z0-9._-]{2,80}$`, no `plugin:` prefix, unique in the registry |
| `name` | 1–60 characters after trimming |
| `version` | valid SemVer, at most 32 characters |
| `kind` | `theme` or `integration` |
| `summary` | 1–200 characters after trimming |
| `author` | 1–100 characters after trimming |
| `homepage` | optional absolute HTTPS URL, no credentials, at most 300 characters |
| `minAppVersion` | optional SemVer; install refuses if newer than the running app |
| `capabilities` | exact match: `["theme.apply"]` for theme, `["integration.inspect"]` for integration |
| `theme` | required for theme, forbidden for integration |
| `integration` | required for integration, forbidden for theme |

Theme payload:

- `scheme`: `dark` or `light`.
- `blurb`: 1–100 characters.
- `tokens`: exactly the 17 colour token names in `ThemeTokens`, no extras, no
  omissions (`scheme` is a sibling field, not a token). Every value is at most
  64 characters and must match one of:
  - `#` plus 3, 6 or 8 hexadecimal digits;
  - `rgba(r, g, b, a)` with numeric channels 0–255, alpha 0–1 (integer or
    decimal), optional whitespace.
- No other colour syntax (named colours, `hsl()`, `var()`, gradients, `url()`)
  is accepted. Values are written to CSS variables only through
  `style.setProperty`, never into markup.

Integration payload:

- `tools`: 1–12 unique ids from the fixed allowlist in `tools.rs`
  (`ollama`, `docker`, `podman`, `gh`, `rust-analyzer`, `claude`, `opencode`,
  `git`, `node`, `python3`, …). Unknown ids are rejected.
- `notes`: 1–2000 characters, plain text rendered as text.
- `docsUrl`: optional absolute HTTPS URL, shown as selectable text; GRAFT does
  not open it.

`minAppVersion` is compared with `env!("CARGO_PKG_VERSION")` using the `semver`
crate. Unparseable versions are rejected.

### Capabilities

Only two capabilities exist in this phase. They describe what the app will do
with the package, not what the package may request:

- `theme.apply` — the package may contribute one theme to the theme catalog.
- `integration.inspect` — the package may add notes and tool references to the
  integrations panel.

A manifest whose capability list does not exactly match its kind is rejected.
There is no wildcard and no capability negotiation. Future kinds add new
capability names in a new manifest schema.

## Registry protocol

A registry is an HTTPS JSON index chosen by the user. There is no built-in
registry, because inventing one would mean showing plugins that do not exist.
The registry URL is stored in the setting `plugins.registryUrl`; an empty value
shows an honest empty state.

The **registry URL** requires HTTPS, except that plain HTTP is accepted for
loopback hosts (`localhost`, `127.0.0.0/8`, `::1`) so a developer can serve a
local index. The loopback exception is scoped to the registry itself: when the
registry URL is remote, every `manifestUrl` and `signatureUrl` in it must also
be HTTPS and non-loopback. A remote registry therefore cannot make GRAFT probe
local services. A loopback registry may point at HTTPS or loopback HTTP
package URLs. URLs with credentials or fragments are rejected. Redirects are
never followed. Timeouts: 20 s connect, 30 s total. Response limits: index
1 MiB, manifest 64 KiB, signature 1 KiB, public key 256 bytes.

The registry is fetched only when the Plugin Marketplace view opens or the
user presses Refresh, never during application boot. Boot reads installed
records and trusted keys from SQLite and applies installed theme plugins
offline; a registry failure is a separate state from an installed-list error.

Index schema 1:

```json
{
  "schema": 1,
  "name": "Example Registry",
  "updatedAt": "2026-11-09T12:00:00Z",
  "plugins": [
    {
      "id": "dev.example.night-owl",
      "name": "Night Owl",
      "version": "1.2.0",
      "kind": "theme",
      "summary": "A low-glare theme for late sessions.",
      "manifestUrl": "https://example.dev/night-owl/plugin.json",
      "manifestSha256": "64 lowercase or uppercase hex characters",
      "signatureUrl": "https://example.dev/night-owl/plugin.json.sig",
      "publicKey": "base64 of 32 raw Ed25519 public-key bytes"
    }
  ]
}
```

Rules:

- At most 500 entries; duplicate ids are rejected.
- Every URL must pass the same HTTPS/loopback and no-credential checks.
- `manifestSha256` must be 64 hex characters.
- `publicKey` must decode to exactly 32 bytes; `fingerprint` is the lowercase
  hex SHA-256 of those bytes and is computed by Rust, never trusted from the
  index.
- The manifest's `id`, `version`, `kind`, `name` and `summary` must equal the
  index entry. A mismatch aborts the install.
- The fetched manifest's SHA-256 must equal `manifestSha256` before JSON
  parsing.
- The index itself is not signed. It is a discovery document, not a trust
  anchor. Trust comes from the package signature and the user's key decision.

`plugin_registry(url)` returns the parsed index with fingerprints attached.
Invalid URLs or malformed entries reject the whole index with a clear error;
no partially trusted data is shown.

## Signing and trust

- Algorithm: Ed25519 (RFC 8032), detached signature over the exact manifest
  bytes. Not a signature over a canonicalised re-serialisation.
- Public key: 32 raw bytes, base64 encoded.
- Signature: 64 raw bytes, base64 encoded, whitespace-trimmed.
- Fingerprint: SHA-256 of the raw public key, lowercase hex.

Trust is trust-on-first-use per key, stored in SQLite:

1. The install review shows the plugin's name, id, version, kind, capabilities,
   and the full fingerprint grouped for reading.
2. A key that is not yet trusted requires an explicit checkbox. The Rust
   command also requires `trustKey = true`; a missing flag is an error, not a
   silent trust. For an already-trusted key `trustKey` is ignored.
   `confirmed = true` is required on every install (including repair and
   upgrade) as the acknowledgement that the review was shown.
3. A trusted key installs without re-asking, but the review still shows the
   fingerprint.
4. If the same plugin id is already installed under a different fingerprint,
   installation is refused with instructions to uninstall first. This prevents
   a registry from silently swapping a plugin's signer.
5. Removing a trusted key is refused while any installed plugin uses it.
6. The app ships no trusted keys and no signing key. Plugin authors sign with
   their own key; the user decides whom to trust.

Unsigned or malformed signatures are always refused.

Schema 1 has exactly one capability per kind, so the pre-install review derives
the capability line from `kind` ("Theme · applies one theme" / "Integration ·
adds notes for detected tools"). No manifest is fetched before the user clicks
Install, no capability is trusted from the index, and there is no preview token
or two-step install. The install command re-validates the manifest and rejects
any kind/capability mismatch.

## Install, update, enable, uninstall

Install verifies everything in Rust in one command, then writes one row:

1. Parse and validate the registry entry.
2. Fetch the manifest with a size cap; verify SHA-256 against the entry.
3. Parse and strictly validate the manifest.
4. Check metadata consistency with the entry and `minAppVersion`.
5. Fetch the signature and public key from the entry; decode and verify the
   signature over the exact manifest bytes.
6. Enforce trust rules and version rules.
7. Insert or update the `plugins` row and add the key to `plugin_keys` when
   `trustKey` is true.

Version rules:

- New id: install.
- Existing id, same fingerprint: allow same version (repair) or a higher
  SemVer. Refuse a lower version.
- Existing id, different fingerprint: refuse; uninstall first.

Enable/disable:

- Disabling sets `enabled = 0`; the theme disappears from the catalog and the
  integration notes are hidden.
- Enabling re-parses the stored manifest, re-verifies the stored signature and
  checks the key is still trusted. A failure keeps the plugin disabled and
  returns the reason.

Uninstall:

- Requires `confirmed = true` in both UI and command; Rust re-checks that the
  plugin exists and the flag is set.
- Deletes the row. The trusted key remains until the user removes it.
- If the uninstalled plugin supplied the active theme, the UI falls back to
  `DEFAULT_THEME_ID` and persists that choice.

`plugin_installed` re-verifies each stored record on read and reports
`verified: false` for records whose signature or trust no longer holds. Those
records are never applied and can only be removed or re-enabled after a
successful verification. This makes local database tampering visible instead of
silently trusted.

## Data model — schema 7

New file `src-tauri/src/schema/007_plugins.sql`, `SCHEMA_VERSION = 7`, with a
migration branch for `current < 7`.

```sql
CREATE TABLE plugins (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  version      TEXT NOT NULL,
  kind         TEXT NOT NULL,
  summary      TEXT NOT NULL DEFAULT '',
  author       TEXT NOT NULL DEFAULT '',
  manifest     TEXT NOT NULL,
  signature    TEXT NOT NULL,
  public_key   TEXT NOT NULL,
  fingerprint  TEXT NOT NULL,
  enabled      INTEGER NOT NULL DEFAULT 1,
  installed_at INTEGER NOT NULL
);

CREATE TABLE plugin_keys (
  fingerprint TEXT PRIMARY KEY,
  public_key  TEXT NOT NULL,
  label       TEXT NOT NULL DEFAULT '',
  added_at    INTEGER NOT NULL
);

CREATE INDEX plugins_kind_idx ON plugins(kind);
```

Notes:

- No secrets are stored here. Keys are public verification material.
- No foreign key from `plugins.fingerprint` to `plugin_keys`, because removing a
  key must be refused in code rather than cascading plugin deletion.
- `manifest` is the exact signed JSON text; `signature` and `public_key` are
  base64. Re-verification reads these three fields.
- `enabled` is `0` or `1`, enforced by Rust.
- `plugin_keys.label` is informational: the manifest `author` of the first
  plugin trusted under that key, trimmed to 100 characters. A key may sign
  plugins by different authors, so the UI also shows `usedBy` (current plugin
  ids). There is no rename command.

## Rust module and IPC

New `src-tauri/src/plugins.rs`, registered in `lib.rs`. Network and validation
stay in Rust; the module returns normalised, public metadata only.

Commands (all errors as `Result<T, String>` following existing modules):

| Command | Arguments | Returns |
|---|---|---|
| `plugin_installed` | none | `PluginRecord[]` |
| `plugin_registry` | `url: String` | `PluginRegistry` |
| `plugin_install` | `entry: RegistryPlugin`, `trustKey: bool`, `confirmed: bool` | `PluginRecord` |
| `plugin_set_enabled` | `id: String`, `enabled: bool` | `PluginRecord` |
| `plugin_uninstall` | `id: String`, `confirmed: bool` | `()` |
| `plugin_keys` | none | `TrustedKey[]` |
| `plugin_remove_key` | `fingerprint: String`, `confirmed: bool` | `()` |

Types (camelCase serialisation):

- `PluginRecord { id, name, version, kind, summary, author, capabilities,
  enabled, verified, fingerprint, installedAt, theme?: ThemePayload,
  integration?: IntegrationPayload }`
- `PluginRegistry { name, updatedAt, plugins: RegistryPlugin[] }`
- `RegistryPlugin { id, name, version, kind, summary, fingerprint, publicKey,
  manifestUrl, manifestSha256, signatureUrl }`
- `TrustedKey { fingerprint, publicKey, label, addedAt, usedBy: string[] }`
- `ThemePayload { scheme, blurb, tokens: ThemeTokens }`
- `IntegrationPayload { tools, notes, docsUrl? }`

Pure validation and verification helpers are separated from the commands so
they can be unit-tested without a webview: URL checks, manifest parsing,
token validation, SHA-256 and signature verification, registry entry checks,
version comparison, and trust/version rules.

Dependencies: `ed25519-dalek` 2, `sha2`, `semver`, `base64` (present), `reqwest`
(present with `json`/`stream`), `serde`/`serde_json` (present). No archive
crate, no WASM runtime, no new Tauri capability.

## Frontend

### State

`src/lib/state/plugins.svelte.ts`:

- `$state` object: `loaded`, `loading`, `error`, `installed`, `registry`,
  `registryUrl`, `keys`, `busyId`, `installError`.
- `loadPlugins()` — boot-safe: loads installed records, trusted keys, and the
  `plugins.registryUrl` setting from local storage only. It performs no network
  request. Used by `App.svelte` before appearance loads.
- `refreshRegistry()` — fetches the saved registry only when the Plugins view
  mounts or Refresh is pressed; keeps `registryLoading`/`registryError`
  separate from the local `error` so an offline registry never blocks installed
  themes.
- `saveRegistryUrl(url)`, `install(entry, trustKey)`,
  `setEnabled(id, enabled)`, `uninstall(id)`, `removeKey(fingerprint)`.
- Persistence of the registry URL goes through `api.setSetting` with a
  debounce; no plugin data is persisted in the webview.

`src/lib/state/themeCatalog.svelte.ts`:

- `allThemes()` merges the built-in `themes` with enabled, verified theme
  plugins, mapping each to `Theme { id: "plugin:" + manifest.id, name,
  blurb, tokens }`.
- `findTheme(id)` falls back to the default built-in theme.
- `reconcileAppearance()` returns the active theme id to the default when the
  selected plugin theme is no longer available.

`appearance.svelte.ts`, `Settings.svelte` and `CommandPalette.svelte` switch
from the static `themes` array to `allThemes()`/`findTheme()`. Boot order in
`App.svelte` loads plugins before appearance so a saved plugin theme resolves
on the first paint.

### Plugin Marketplace view

New `src/views/Plugins.svelte`, routed for `ui.view === "plugins"` in
`App.svelte`; nav entry status changes from `planned` to `ready`.

Layout, using existing tokens and primitives:

```
Plugin Marketplace                                  Secured by signatures
Registry  [ https://…                         ]  [Save] [Refresh]
          last updated 2026-11-09T12:00:00Z · 3 plugins

Installed (2)
  Night Owl          theme · v1.2.0 · verified · enabled   [Disable] [Remove]
  Ollama notes       integration · v1.0.0 · verified       [Disable] [Remove]

Browse · Example Registry
  Another theme      theme · v2.0.0 · proof…                [Install]
    ↳ review panel: capabilities, signer fingerprint,
      [ ] I trust this key, [Install]

Trusted keys (1)
  ed25519 9f2c 41ab …  · Example Author                      [Remove]

Integrations on this machine
  Ollama           not found    · used by the AI workspace
  Docker           27.4.1       · container tooling
  GitHub CLI       2.63.2       · repository operations
  rust-analyzer    not found    · language server
  Claude Code      not found
  OpenCode         not found
```

Details:

- Registry URL input plus Save/Refresh; empty registry shows a plain-language
  empty state with an inline, collapsible copy of the registry schema (the
  webview cannot open local files), never sample plugins.
- Installed list grouped by kind, enabled state toggle, verified badge, remove
  with confirmation. Removing a theme reconciles the active theme.
- Browse list from the registry with install action; selection opens a review
  panel showing name/id/version/kind, capabilities, summary, fingerprint, and
  the trust checkbox when the key is new. Installing is disabled until the
  review is acknowledged.
- Trusted keys list with full fingerprint, label, plugins using the key, and
  removal with confirmation; removal is refused while in use.
- Integrations panel calls `detect_tools()` and shows real status for the Phase
  6 set: Ollama, Docker, `gh`, rust-analyzer, Claude Code, OpenCode. Installed
  integration plugins add their notes beneath referenced tools. Missing tools
  say "not found" and show no invented version.
- The footer states the model: "Plugins are signed data. GRAFT never runs
  plugin code."

### Navigation and docs

- `src/lib/nav.ts`: plugins entry becomes `status: "ready"` with bullets that
  describe signed declarative packages, capability review, and the real tool
  panel.
- `docs/ARCHITECTURE.md`: add `plugins.rs`, schema 7 tables, the
  `plugins.registryUrl` setting, all seven IPC rows, and the security posture
  (signature verification, trust-on-first-use, no code execution).
- `docs/PLAN.md`: Phase 6 status and delivered scope; note executable plugins
  are explicitly deferred.
- `README.md`: plugin marketplace is no longer "scheduled".
- Add `docs/plugins/registry.md` with the manifest and registry schemas so a
  third party can publish without reading Rust. The same schema is available
  inline in the empty state; the app never opens local files.

## Security model

| Threat | Mitigation |
|---|---|
| Malicious manifest | 64 KiB cap, strict schema, `deny_unknown_fields`, id/name/length rules |
| CSS injection through theme values | fixed 17-token vocabulary, strict hex/rgba grammar, CSS variables only |
| Tampered package | SHA-256 over exact bytes before parsing; Ed25519 signature over exact bytes |
| Signature/key substitution | fingerprint shown; new key needs explicit trust; key change for installed id refused |
| Registry compromise | registry is discovery only; signatures and user trust are the anchor |
| Registry SSRF / redirect abuse | registry HTTPS or loopback HTTP; remote registries cannot point package URLs at loopback; no credentials, no fragments, no redirects, size/time caps |
| Downgrade / replay | SemVer monotonicity per id; same-version repair only with same key |
| Database tampering | re-verify stored manifest/signature/trust on read; invalid records are not applied |
| Destructive actions | uninstall and key removal require `confirmed = true` and Rust revalidation |
| Plugin persistence | no files written, no archives, no paths; rows in SQLite only |

There is no secret material in a plugin, no token is read or written by plugin
code, and no plugin can call an AI provider or GitHub.

## Testing

Rust unit tests (pure helpers plus database tests):

1. Valid theme and integration manifests parse; metadata round-trips.
2. Unknown manifest fields, invalid ids, oversized strings and bad SemVer are
   rejected.
3. Kind/capability mismatches and duplicate capabilities are rejected.
4. Theme token validation rejects missing/extra tokens, named colours, `url()`,
   `var()`, gradients and oversized values; accepts hex and rgba.
5. Ed25519 sign/verify round-trip with a deterministic test key; tampered
   manifest and wrong key fail.
6. SHA-256 mismatch is rejected before parsing.
7. Registry parsing enforces size, entry count, duplicate ids, URL rules and
   key length; fingerprints are computed correctly.
8. Manifest/index metadata inconsistency aborts install.
9. Timeout/redirect policy and URL validation helpers reject non-loopback HTTP
   and credentials; a remote registry entry pointing at loopback HTTP is
   rejected, while a loopback registry may use loopback package URLs.
10. Version rules: upgrade allowed, equal repair allowed with same key,
    downgrade refused, key change refused.
11. Trust rules: new key without `trustKey` refused; trusted key accepted with
    either `trustKey` value; every install without `confirmed` refused;
    removing an in-use key refused.
12. Boot/regression: loading installed plugins performs no network request;
    registry errors leave installed themes available.
12. Database: install/enable/uninstall round-trip; uninstall needs
    `confirmed`; re-verification marks tampered rows unverified.
13. Migration 7 creates both tables and the index; re-opening is idempotent.

Frontend verification: `npm run check`, `npm run build`, and a manual pass with
a loopback registry: install a theme, confirm it appears in Settings and
applies, uninstall it, confirm fallback, install an integration plugin and see
real detected-tool status. `npm run verify` and `cargo test` must be green.

Live registry testing is not available in CI; the tests use in-process
fixtures and a deterministic key. The final report must say this explicitly.

## Acceptance criteria

- No code path executes plugin content or spawns a process for a plugin.
- Unsigned, corrupted, mismatched, downgraded or untrusted packages cannot be
  installed.
- A new key always requires an explicit user decision; a key change for an
  installed plugin is refused.
- Installed theme plugins appear in Settings, apply through CSS variables, and
  fall back cleanly when disabled or removed.
- Integration plugins can only reference known tool ids; the panel shows real
  detection results for the Phase 6 tools.
- All commands have `api.ts` wrappers and IPC table rows; schema 7 is
  documented with a migration.
- `npm run verify` and `cargo test` pass; the report distinguishes unit-tested
  behaviour from the unverified live-registry path.

## What is left to the builder

- Exact view styling and copy within the existing token system (no colour
  literals).
- Whether the integrations panel shows only the Phase 6 set or all detected
  tools with the Phase 6 set pinned; the design assumes the Phase 6 set plus a
  compact "other tools" count.
- Error message wording and grouping.
- The exact contents of `docs/plugins/registry.md` and an example registry
  fixture used only by tests.
- Dependency patch versions (`ed25519-dalek` 2.x, `sha2` 0.10.x, `semver`
  1.x).
- Whether to expose a "copy fingerprint" action; not required.
- Future work explicitly out of scope: WASM plugin runtime, manual URL install
  without a registry, plugin updates, a hosted registry, plugin-defined
  commands or panels.
