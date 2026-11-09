# Plugin Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Phase 6 as a signed, declarative plugin marketplace: verified theme and integration packages, a user-supplied registry, explicit key trust, a real Plugins view, and plugin themes in the existing theme system.

**Architecture:** Rust owns registry fetching, manifest validation, Ed25519/SHA-256 verification, trust rules and SQLite schema 7. The webview receives only normalised metadata through `api.ts`; no plugin code, process, archive or filesystem path is involved. Installed themes merge into the existing CSS-variable theme catalog.

**Tech Stack:** Tauri 2, Rust, reqwest 0.13, rusqlite, ed25519-dalek 2, sha2 0.10, semver 1, Svelte 5 runes, TypeScript.

**Spec:** `docs/superpowers/specs/2026-11-09-plugin-marketplace-design.md`

## Global Constraints

- No plugin code execution, process spawning, archive extraction, arbitrary file reads, or plugin-controlled network requests.
- Manifest: UTF-8 JSON, schema `1`, at most 64 KiB, unknown fields rejected. Signature text at most 1 KiB. Public key at most 256 bytes and exactly 32 decoded bytes.
- Registry index: at most 1 MiB, at most 500 entries, schema `1`, unique ids, no redirects, 20 s connect / 30 s total timeout.
- Registry URL: HTTPS, or loopback HTTP for development. A remote registry's package URLs must be HTTPS and non-loopback. Credentials and fragments are rejected.
- Theme payload: exactly the 17 colour tokens in `ThemeTokens`; each value at most 64 characters and only `#rgb`, `#rrggbb`, `#rrggbbaa`, or `rgba(r,g,b,a)`.
- Capabilities exactly: `["theme.apply"]` for `theme`, `["integration.inspect"]` for `integration`.
- Trust is per public key, trust-on-first-use. New keys require `trustKey = true`; every install requires `confirmed = true`; a key change for an existing id is refused until uninstall.
- Schema 7 only: `SCHEMA_VERSION = 7`, new `src-tauri/src/schema/007_plugins.sql`, migration branch `current < 7`. Never edit a shipped migration.
- Every command needs a handler, `lib.rs` registration, `api.ts` wrapper and an `ARCHITECTURE.md` IPC row.
- No colour literals in Svelte components; use theme tokens. Svelte 5 runes and `onclick`, not `on:click`.
- All network and filesystem access stays in Rust. Plugin keys are public verification material only; no secret is stored in SQLite.
- Finish every task with `cargo test --manifest-path src-tauri/Cargo.toml` (or the task's named frontend check) green before committing.

## Review Focus

These are the inputs most likely to break a real user. Each is pinned by a test or a named manual check in the task that owns it.

1. **Remote registry points a package URL at loopback.** It must be rejected before any request; a loopback registry may still use loopback package URLs. (Task 3 test.)
2. **An installed database row is edited after install.** It must report `verified: false`, expose no theme payload, and never apply the theme. (Task 4 test.)
3. **The active plugin theme is disabled, uninstalled or fails verification.** Appearance must fall back to `graft` and persist that choice. (Task 9 manual check; no frontend test runner exists.)
4. **The registry is offline, redirects, or returns an oversized body.** The Plugins view must show a separate registry error while installed themes still load, and boot must not wait for the registry. (Task 5 size/redirect tests, Task 7 state separation, Task 9 manual check.)
5. **A signature or key contains whitespace, malformed base64 or trailing bytes.** It must be rejected without panics or truncation. (Task 3 test.)

---

### Task 1: Schema 7, dependencies and tool allowlist

**Files:**
- Modify: `src-tauri/Cargo.toml:16-26`
- Create: `src-tauri/src/schema/007_plugins.sql`
- Modify: `src-tauri/src/db.rs:7`, `src-tauri/src/db.rs:161-164`
- Modify: `src-tauri/src/tools.rs:20-39`
- Test: `src-tauri/src/db.rs` tests, `src-tauri/src/tools.rs` tests

**Interfaces:**
- Produces: `SCHEMA_VERSION == 7`; tables `plugins` and `plugin_keys`; `tools::is_known_tool(&str) -> bool`; dependency crates `ed25519-dalek`, `sha2`, `semver`.

- [ ] **Step 1: Write the failing migration test**

Add to `db.rs` tests:

```rust
#[test]
fn migration_creates_plugin_tables() {
    let path = temp_path("plugin-schema");
    let db = Db::open(&path).expect("open");
    let conn = db.conn();
    let count = |name: &str| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
            [name],
            |row| row.get(0),
        )
        .expect("count sqlite objects")
    };
    assert_eq!(count("plugins"), 1);
    assert_eq!(count("plugin_keys"), 1);
    assert_eq!(count("plugins_kind_idx"), 1);
    cleanup(&path);
}
```

- [ ] **Step 2: Run it and verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::tests::migration_creates_plugin_tables`
Expected: FAIL, with `left: 0` for `plugins` (or an equivalent assertion failure).

- [ ] **Step 3: Add schema 7 and the dependency set**

Create `src-tauri/src/schema/007_plugins.sql`:

```sql
-- GRAFT schema 007
-- Signed declarative plugins. Public verification material only; no secrets.

CREATE TABLE IF NOT EXISTS plugins (
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

CREATE TABLE IF NOT EXISTS plugin_keys (
    fingerprint TEXT PRIMARY KEY,
    public_key  TEXT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',
    added_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS plugins_kind_idx ON plugins(kind);
```

In `db.rs`, set `SCHEMA_VERSION` to `7` and add the migration branch after migration 006:

```rust
if current < 7 {
    conn.execute_batch(include_str!("schema/007_plugins.sql"))
        .map_err(|e| format!("migration 007 failed: {e}"))?;
}
```

Add to `[dependencies]` in `Cargo.toml`:

```toml
ed25519-dalek = "2"
sha2 = "0.10"
semver = "1"
```

- [ ] **Step 4: Run the migration test and verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::tests::migration_creates_plugin_tables`
Expected: PASS.

- [ ] **Step 5: Write the failing tool allowlist test**

Add a `#[cfg(test)] mod tests` at the end of `tools.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_phase_six_tools() {
        for id in ["ollama", "docker", "gh", "rust-analyzer", "claude", "opencode"] {
            assert!(is_known_tool(id), "{id} is missing from the tool allowlist");
        }
        assert!(!is_known_tool("definitely-not-a-tool"));
    }
}
```

- [ ] **Step 6: Run it and verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml tools::tests::knows_the_phase_six_tools`
Expected: FAIL to compile with `cannot find function 'is_known_tool'`.

- [ ] **Step 7: Add the missing tools and helper**

Append these `ToolSpec` rows to `TOOLS` (order after the existing entries):

```rust
ToolSpec { id: "ollama", name: "Ollama", version_args: &["--version"] },
ToolSpec { id: "rust-analyzer", name: "rust-analyzer", version_args: &["--version"] },
ToolSpec { id: "claude", name: "Claude Code", version_args: &["--version"] },
ToolSpec { id: "opencode", name: "OpenCode", version_args: &["--version"] },
```

Add:

```rust
pub fn is_known_tool(id: &str) -> bool {
    TOOLS.iter().any(|spec| spec.id == id)
}
```

- [ ] **Step 8: Run the tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS, including both new tests.

```bash
git add src-tauri/Cargo.toml src-tauri/src/db.rs src-tauri/src/schema/007_plugins.sql src-tauri/src/tools.rs
git commit -m "feat: add plugin schema and tool detection"
```

---

### Task 2: Manifest model and strict validation

**Files:**
- Create: `src-tauri/src/plugins.rs`
- Modify: `src-tauri/src/lib.rs:1-13` (add `mod plugins;`)
- Test: `src-tauri/src/plugins.rs` tests

**Interfaces:**
- Produces: `PluginKind { Theme, Integration }`, `ThemePayload`, `ThemeTokens`, `IntegrationPayload`; `parse_manifest(&[u8]) -> Result<Manifest, String>`; `valid_colour(&str) -> bool`.
- `ThemeTokens` serialises with camelCase field names. `ThemePayload { scheme, blurb, tokens }`.

- [ ] **Step 1: Write the failing manifest tests**

In a new `plugins.rs`, add types and a test module. The tests use a fixture with the full 17-token payload:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn theme_value() -> Value {
        json!({
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
                    "bg": "#0b0d12", "bgElev": "#0f1218", "surface": "#121620",
                    "surfaceAlt": "#171c28", "border": "#232a3a", "borderStrong": "#333d52",
                    "text": "#e2e6ee", "textMuted": "#9aa3b4", "textFaint": "#6a7384",
                    "accent": "#d9a441", "accentSoft": "rgba(217, 164, 65, 0.14)",
                    "accentInk": "#15120a", "ok": "#79b47a", "warn": "#d9a441",
                    "danger": "#d16a5f", "info": "#7ba7c4", "shadow": "rgba(0, 0, 0, 0.45)"
                }
            }
        })
    }

    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("serialise fixture")
    }

    #[test]
    fn parses_valid_theme_and_integration_manifests() {
        let theme = parse_manifest(&bytes(&theme_value())).expect("valid theme");
        assert_eq!(theme.kind, PluginKind::Theme);
        assert!(theme.theme.is_some());
        assert!(theme.integration.is_none());

        let mut integration = theme_value();
        integration["kind"] = json!("integration");
        integration["capabilities"] = json!(["integration.inspect"]);
        integration.as_object_mut().unwrap().remove("theme");
        integration["integration"] = json!({
            "tools": ["ollama", "docker"],
            "notes": "Start Ollama before using local models.",
            "docsUrl": "https://example.dev/notes"
        });
        let parsed = parse_manifest(&bytes(&integration)).expect("valid integration");
        assert_eq!(parsed.kind, PluginKind::Integration);
        assert!(parsed.integration.is_some());
    }

    #[test]
    fn rejects_unknown_fields_bad_ids_and_invalid_metadata() {
        let mut value = theme_value();
        value["extra"] = json!(true);
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["id"] = json!("Plugin:Bad");
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["version"] = json!("1.0");
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["minAppVersion"] = json!("not-a-version");
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["summary"] = json!("");
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["homepage"] = json!("http://example.dev");
        assert!(parse_manifest(&bytes(&value)).is_err());
    }

    #[test]
    fn rejects_kind_and_capability_mismatches() {
        let mut value = theme_value();
        value["capabilities"] = json!(["integration.inspect"]);
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["capabilities"] = json!(["theme.apply", "theme.apply"]);
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value.as_object_mut().unwrap().remove("theme");
        assert!(parse_manifest(&bytes(&value)).is_err());
    }

    #[test]
    fn validates_colours_and_the_exact_token_set() {
        for good in [
            "#abc",
            "#0b0d12",
            "#0b0d12ff",
            "rgba(217, 164, 65, 0.14)",
            "rgba(0,0,0,1)",
        ] {
            assert!(valid_colour(good), "{good} should be valid");
        }
        for bad in [
            "red",
            "url(https://example.dev/x)",
            "var(--accent)",
            "linear-gradient(#000,#fff)",
            "rgba(256, 0, 0, 1)",
            "rgba(0, 0, 0, 1.1)",
            "#12",
            "#gggggg",
        ] {
            assert!(!valid_colour(bad), "{bad} should be invalid");
        }

        let mut value = theme_value();
        value["theme"]["tokens"]["evil"] = json!("#000000");
        assert!(parse_manifest(&bytes(&value)).is_err());

        let mut value = theme_value();
        value["theme"]["tokens"].as_object_mut().unwrap().remove("shadow");
        assert!(parse_manifest(&bytes(&value)).is_err());
    }

    #[test]
    fn rejects_integration_tools_outside_the_allowlist() {
        let mut value = theme_value();
        value["kind"] = json!("integration");
        value["capabilities"] = json!(["integration.inspect"]);
        value.as_object_mut().unwrap().remove("theme");
        value["integration"] = json!({
            "tools": ["ollama", "definitely-not-a-tool"],
            "notes": "Some notes."
        });
        assert!(parse_manifest(&bytes(&value)).is_err());

        value["integration"]["tools"] = json!(["ollama", "ollama"]);
        assert!(parse_manifest(&bytes(&value)).is_err());

        value["integration"]["tools"] = json!(["ollama"]);
        value["integration"]["notes"] = json!("");
        assert!(parse_manifest(&bytes(&value)).is_err());
    }
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: FAIL to compile because the types and `parse_manifest` do not exist.

- [ ] **Step 3: Implement the manifest model and validation**

Add `mod plugins;` to `lib.rs`. In `plugins.rs` implement:

```rust
use serde::{Deserialize, Serialize};

const MANIFEST_MAX_BYTES: usize = 64 * 1024;
const MAX_TOKEN_CHARS: usize = 64;
const TOKEN_COUNT: usize = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind { Theme, Integration }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemePayload {
    pub scheme: String,
    pub blurb: String,
    pub tokens: ThemeTokens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeTokens {
    pub bg: String, pub bg_elev: String, pub surface: String, pub surface_alt: String,
    pub border: String, pub border_strong: String, pub text: String,
    pub text_muted: String, pub text_faint: String, pub accent: String,
    pub accent_soft: String, pub accent_ink: String, pub ok: String,
    pub warn: String, pub danger: String, pub info: String, pub shadow: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegrationPayload {
    pub tools: Vec<String>,
    pub notes: String,
    #[serde(default)]
    pub docs_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    pub summary: String,
    pub author: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: Option<String>,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub theme: Option<ThemePayload>,
    #[serde(default)]
    pub integration: Option<IntegrationPayload>,
}
```

Implement `parse_manifest`, `validate_manifest`, `valid_id`, `valid_colour`, `validate_https_url`, and length helpers. Requirements:

- Reject bytes over 64 KiB before parsing, and invalid JSON/unknown fields.
- `schema == 1`; `id` matches `^[a-z0-9][a-z0-9._-]{2,80}$`; name 1–60, summary 1–200, author 1–100 characters after trimming; version valid SemVer and ≤32 characters.
- `homepage` and `docsUrl` are optional absolute HTTPS URLs, no credentials or fragments, ≤300 characters.
- `minAppVersion` is optional valid SemVer, ≤32 characters, and must not be newer than `env!("CARGO_PKG_VERSION")` using `semver::Version::parse`.
- Capabilities are exactly `["theme.apply"]` for theme and `["integration.inspect"]` for integration; no extras, duplicates or wildcards.
- Theme requires `theme` and forbids `integration`; integration is the reverse.
- Theme `scheme` is `dark` or `light`; blurb 1–100; every one of the 17 token fields is present and valid.
- Integration `tools` has 1–12 unique ids accepted by `tools::is_known_tool`; `notes` is 1–2000 characters.
- `valid_colour` accepts only `#` + 3/6/8 hex digits, or exactly four comma-separated `rgba` components: channels 0–255, alpha 0–1, no signs or exponent, optional whitespace.

- [ ] **Step 4: Run the tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: PASS, all manifest tests.

```bash
git add src-tauri/src/lib.rs src-tauri/src/plugins.rs
git commit -m "feat: validate signed plugin manifests"
```

---

### Task 3: Signatures, registry parsing and version rules

**Files:**
- Modify: `src-tauri/src/plugins.rs`
- Test: `src-tauri/src/plugins.rs` tests

**Interfaces:**
- Produces: `PluginRegistry`, `RegistryPlugin`, `sha256_hex`, `fingerprint`, `verify_manifest_signature`, `validate_registry_url`, `validate_package_url`, `parse_registry`, `check_entry_matches`, `check_version`.
- `RegistryPlugin` includes `fingerprint: String`; the index's raw entry type does not.

- [ ] **Step 1: Write the failing crypto, URL and registry tests**

Append to `plugins.rs` tests:

```rust
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};

fn key_b64(key: &SigningKey) -> String {
    STANDARD.encode(key.verifying_key().to_bytes())
}

fn entry_for(manifest: &[u8], public_key: &str) -> RegistryPlugin {
    let parsed = parse_manifest(manifest).expect("manifest for registry entry");
    RegistryPlugin {
        id: parsed.id,
        name: parsed.name,
        version: parsed.version,
        kind: parsed.kind,
        summary: parsed.summary,
        manifest_url: "https://example.dev/night-owl/plugin.json".into(),
        manifest_sha256: sha256_hex(manifest),
        signature_url: "https://example.dev/night-owl/plugin.json.sig".into(),
        public_key: public_key.into(),
        fingerprint: fingerprint(&STANDARD.decode(public_key).unwrap()),
    }
}

fn registry_value(manifest_url: &str) -> Value {
    json!({
        "schema": 1,
        "name": "Example Registry",
        "updatedAt": "2026-11-09T12:00:00Z",
        "plugins": [{
            "id": "dev.example.night-owl",
            "name": "Night Owl",
            "version": "1.2.0",
            "kind": "theme",
            "summary": "A low-glare theme for late sessions.",
            "manifestUrl": manifest_url,
            "manifestSha256": "a".repeat(64),
            "signatureUrl": "https://example.dev/night-owl/plugin.json.sig",
            "publicKey": STANDARD.encode([9u8; 32])
        }]
    })
}

#[test]
fn verifies_ed25519_and_rejects_tampering() {
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let manifest = br#"{"schema":1}"#;
    let signature = STANDARD.encode(key.sign(manifest).to_bytes());
    let public_key = key_b64(&key);

    assert!(verify_manifest_signature(manifest, &signature, &public_key).is_ok());
    assert!(verify_manifest_signature(b"{}", &signature, &public_key).is_err());

    let other = SigningKey::from_bytes(&[8u8; 32]);
    assert!(verify_manifest_signature(manifest, &signature, &key_b64(&other)).is_err());
    assert!(verify_manifest_signature(manifest, "not base64!", &public_key).is_err());
    assert!(verify_manifest_signature(manifest, &signature, "short").is_err());
    assert!(verify_manifest_signature(manifest, &format!(" {signature} "), &public_key).is_ok());
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn validates_registry_and_package_urls() {
    assert!(validate_registry_url("https://example.com/index.json").is_ok());
    assert!(validate_registry_url("http://localhost:8080/index.json").is_ok());
    assert!(validate_registry_url("http://example.com/index.json").is_err());
    assert!(validate_registry_url("https://user:pass@example.com/x").is_err());
    assert!(validate_registry_url("https://example.com/x#frag").is_err());
    assert!(validate_package_url("http://127.0.0.1:8080/p.json", false).is_err());
    assert!(validate_package_url("http://127.0.0.1:8080/p.json", true).is_ok());
}

#[test]
fn remote_registries_cannot_point_packages_at_loopback() {
    let remote = serde_json::to_vec(&registry_value("http://127.0.0.1:8000/plugin.json")).unwrap();
    assert!(parse_registry(&remote, "https://example.com/index.json").is_err());

    let loopback = serde_json::to_vec(&registry_value("http://127.0.0.1:8000/plugin.json")).unwrap();
    let parsed = parse_registry(&loopback, "http://127.0.0.1:9000/index.json").expect("loopback registry");
    assert_eq!(parsed.plugins[0].fingerprint, fingerprint(&[9u8; 32]));
}

#[test]
fn rejects_duplicate_ids_and_bad_registry_entries() {
    let mut value = registry_value("https://example.dev/plugin.json");
    let duplicate = value["plugins"][0].clone();
    value["plugins"].as_array_mut().unwrap().push(duplicate);
    assert!(parse_registry(&serde_json::to_vec(&value).unwrap(), "https://example.com/index.json").is_err());

    let mut value = registry_value("https://example.dev/plugin.json");
    value["plugins"][0]["publicKey"] = json!("short");
    assert!(parse_registry(&serde_json::to_vec(&value).unwrap(), "https://example.com/index.json").is_err());

    let mut value = registry_value("https://example.dev/plugin.json");
    value["plugins"][0]["manifestSha256"] = json!("not-hex");
    assert!(parse_registry(&serde_json::to_vec(&value).unwrap(), "https://example.com/index.json").is_err());
}

#[test]
fn enforces_metadata_and_version_rules() {
    let manifest = parse_manifest(&bytes(&theme_value())).unwrap();
    let entry = entry_for(&bytes(&theme_value()), &STANDARD.encode([9u8; 32]));
    assert!(check_entry_matches(&manifest, &entry).is_ok());

    let mut mismatch = entry.clone();
    mismatch.version = "2.0.0".into();
    assert!(check_entry_matches(&manifest, &mismatch).is_err());

    assert!(check_version(None, None, "1.0.0", "aa").is_ok());
    assert!(check_version(Some("1.0.0"), Some("aa"), "1.1.0", "aa").is_ok());
    assert!(check_version(Some("1.0.0"), Some("aa"), "1.0.0", "aa").is_ok());
    assert!(check_version(Some("1.1.0"), Some("aa"), "1.0.0", "aa").is_err());
    assert!(check_version(Some("1.0.0"), Some("aa"), "1.1.0", "bb").is_err());
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: FAIL to compile because the crypto, URL and registry functions do not exist.

- [ ] **Step 3: Implement signing, hashing, URLs and registry parsing**

Add:

```rust
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistry {
    pub name: String,
    pub updated_at: String,
    pub plugins: Vec<RegistryPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    pub summary: String,
    pub manifest_url: String,
    pub manifest_sha256: String,
    pub signature_url: String,
    pub public_key: String,
    pub fingerprint: String,
}
```

Use a private `RegistryEntry` with the same fields except `fingerprint` for index deserialisation, then map it into `RegistryPlugin`. Implement:

- `sha256_hex(&[u8]) -> String` lowercase hex; `fingerprint(&[u8]) -> String` is the same over the raw public key.
- `decode_public_key(&str) -> Result<[u8; 32], String>` trims surrounding whitespace, rejects input over 256 bytes, uses standard base64, requires exactly 32 bytes.
- `verify_manifest_signature(manifest: &[u8], signature_b64: &str, public_key_b64: &str) -> Result<(), String>` trims whitespace, decodes exactly 64 signature bytes, and verifies with `VerifyingKey`.
- `validate_registry_url(&str) -> Result<reqwest::Url, String>`: absolute URL, length ≤2048, HTTPS or loopback HTTP, no credentials or fragments, has a host.
- `validate_package_url(&str, allow_loopback_http: bool) -> Result<reqwest::Url, String>`: same checks; when `allow_loopback_http` is false, reject loopback hosts and plain HTTP.
- `is_loopback_url(&reqwest::Url) -> bool`: `localhost`, any `127.0.0.0/8` IPv4, or `::1`.
- `parse_registry(bytes: &[u8], registry_url: &str) -> Result<PluginRegistry, String>`: cap 1 MiB, schema `1`, name 1–100, updatedAt 1–40, at most 500 entries, unique ids, validate every entry's metadata and URLs in the context of the registry, decode keys, compute fingerprints, and reject the whole index on any error.
- `check_entry_matches(&Manifest, &RegistryPlugin) -> Result<(), String>`: exact equality for id, version, kind, name and summary.
- `check_version(existing_version: Option<&str>, existing_fingerprint: Option<&str>, version: &str, fingerprint: &str) -> Result<(), String>`: new id accepted; different fingerprint refused; same fingerprint accepts equal or higher SemVer, refuses lower.
- `check_min_app_version(Option<&str>) -> Result<(), String>`: parse both versions and refuse when the manifest's minimum is newer than `env!("CARGO_PKG_VERSION")`.

- [ ] **Step 4: Run the tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: PASS, including the loopback SSRF and malformed-base64 cases.

```bash
git add src-tauri/src/plugins.rs
git commit -m "feat: verify plugin signatures and registries"
```

---

### Task 4: Persistence, trust and local commands

**Files:**
- Modify: `src-tauri/src/plugins.rs`
- Modify: `src-tauri/src/lib.rs:33-126` (register five commands)
- Test: `src-tauri/src/plugins.rs` tests

**Interfaces:**
- Produces: `PluginRecord`, `TrustedKey`, `install_verified`, `read_installed`, `read_keys`, `set_plugin_enabled`, `uninstall_plugin`, `remove_trusted_key`; commands `plugin_installed`, `plugin_set_enabled`, `plugin_uninstall`, `plugin_keys`, `plugin_remove_key`.

- [ ] **Step 1: Write the failing persistence tests**

Add these helpers and tests:

```rust
fn temp_db(label: &str) -> (std::path::PathBuf, Db) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "graft-plugins-{label}-{}-{nanos}.db",
        std::process::id()
    ));
    let db = Db::open(&path).expect("open database");
    (path, db)
}

fn cleanup(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

fn signed_theme(version: &str, key: &SigningKey) -> (Vec<u8>, String, String) {
    let mut value = theme_value();
    value["version"] = json!(version);
    let manifest = bytes(&value);
    let signature = STANDARD.encode(key.sign(&manifest).to_bytes());
    (manifest, signature, key_b64(key))
}

#[test]
fn installs_reads_and_reverifies_themes() {
    let (path, db) = temp_db("install");
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let (manifest, signature, public_key) = signed_theme("1.0.0", &key);
    let entry = entry_for(&manifest, &public_key);

    let record = install_verified(&db, &entry, &manifest, &signature, true, true).expect("install");
    assert!(record.verified && record.enabled);
    assert_eq!(record.fingerprint, fingerprint(&key.verifying_key().to_bytes()));
    assert!(record.theme.is_some());
    assert_eq!(read_installed(&db).expect("list").len(), 1);
    assert_eq!(read_keys(&db).expect("keys").len(), 1);

    cleanup(&path);
}

#[test]
fn requires_confirmation_and_explicit_first_trust() {
    let (path, db) = temp_db("trust");
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let (manifest, signature, public_key) = signed_theme("1.0.0", &key);
    let entry = entry_for(&manifest, &public_key);

    assert!(install_verified(&db, &entry, &manifest, &signature, false, true).is_err());
    assert!(install_verified(&db, &entry, &manifest, &signature, true, false).is_err());
    install_verified(&db, &entry, &manifest, &signature, true, true).expect("first trust");
    install_verified(&db, &entry, &manifest, &signature, false, true).expect("trusted key");

    cleanup(&path);
}

#[test]
fn refuses_downgrades_and_silent_key_changes() {
    let (path, db) = temp_db("versions");
    let first = SigningKey::from_bytes(&[7u8; 32]);
    let second = SigningKey::from_bytes(&[8u8; 32]);

    let (manifest, signature, public_key) = signed_theme("1.1.0", &first);
    install_verified(&db, &entry_for(&manifest, &public_key), &manifest, &signature, true, true)
        .expect("initial install");

    let (old_manifest, old_signature, old_key) = signed_theme("1.0.0", &first);
    assert!(install_verified(
        &db,
        &entry_for(&old_manifest, &old_key),
        &old_manifest,
        &old_signature,
        false,
        true,
    )
    .is_err());

    let (new_manifest, new_signature, new_key) = signed_theme("2.0.0", &second);
    assert!(install_verified(
        &db,
        &entry_for(&new_manifest, &new_key),
        &new_manifest,
        &new_signature,
        true,
        true,
    )
    .is_err());

    cleanup(&path);
}

#[test]
fn tampered_rows_are_unverified_and_enable_fails() {
    let (path, db) = temp_db("tamper");
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let (manifest, signature, public_key) = signed_theme("1.0.0", &key);
    let record = install_verified(
        &db,
        &entry_for(&manifest, &public_key),
        &manifest,
        &signature,
        true,
        true,
    )
    .expect("install");

    set_plugin_enabled(&db, &record.id, false).expect("disable");
    conn(&db)
        .execute("UPDATE plugins SET signature = 'AAAA' WHERE id = ?1", [&record.id])
        .expect("tamper");
    let listed = read_installed(&db).expect("list");
    assert!(!listed[0].verified);
    assert!(listed[0].theme.is_none());
    assert!(set_plugin_enabled(&db, &record.id, true).is_err());

    cleanup(&path);
}

#[test]
fn removal_requires_confirmation_and_refuses_in_use_keys() {
    let (path, db) = temp_db("remove");
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let (manifest, signature, public_key) = signed_theme("1.0.0", &key);
    let record = install_verified(
        &db,
        &entry_for(&manifest, &public_key),
        &manifest,
        &signature,
        true,
        true,
    )
    .expect("install");

    assert!(remove_trusted_key(&db, &record.fingerprint, true).is_err());
    assert!(uninstall_plugin(&db, &record.id, false).is_err());
    uninstall_plugin(&db, &record.id, true).expect("uninstall");
    assert!(remove_trusted_key(&db, &record.fingerprint, false).is_err());
    remove_trusted_key(&db, &record.fingerprint, true).expect("remove key");

    cleanup(&path);
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: FAIL to compile because the persistence functions do not exist.

- [ ] **Step 3: Implement the schema-7 persistence and commands**

Add public types:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    pub summary: String,
    pub author: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub verified: bool,
    pub fingerprint: String,
    pub installed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<IntegrationPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedKey {
    pub fingerprint: String,
    pub public_key: String,
    pub label: String,
    pub added_at: i64,
    pub used_by: Vec<String>,
}
```

Implement a private `StoredPlugin` row (all columns) and:

- `conn(&Db)` using the same poisoned-lock recovery as `ai.rs`.
- `read_keys`: read all keys, attach `used_by` by querying `plugins.fingerprint`.
- `read_installed`: read all rows, read the trusted fingerprint set, and call `record_from_row`.
- `read_plugin`: read one row by id; error `"Plugin not found."` if absent.
- `verify_row(row, trusted) -> Result<Manifest, String>`: decode the stored key, check the stored fingerprint equals `fingerprint(raw_key)`, parse the stored manifest, verify its signature, require manifest id/name/version/kind/summary/author to equal the columns, and require `trusted`.
- `record_from_row`: when verification fails, return `verified: false`, derive capabilities from the stored kind, and omit theme/integration. When it succeeds, return `verified: true` with the manifest payload.
- `install_verified(db, entry, manifest_bytes, signature_b64, trust_key, confirmed)`: require `confirmed`; compare SHA-256 with `entry.manifest_sha256` before parsing; parse the manifest; call `check_entry_matches` and `check_min_app_version`; recompute the key fingerprint and require it equals `entry.fingerprint`; verify the signature; look up the existing row and call `check_version`; require the key to be trusted unless `trust_key`; then insert the key with `INSERT OR IGNORE` (label = trimmed author, max 100) and upsert the plugin in one transaction, preserving `enabled` and `installed_at` on conflict. Return `read_plugin`.
- `set_plugin_enabled`: read the row; when enabling, call `verify_row` and propagate failure without changing the row; update `enabled`; return the refreshed record.
- `uninstall_plugin(db, id, confirmed)`: require `confirmed`, delete, error if no row matched.
- `remove_trusted_key(db, fingerprint, confirmed)`: require `confirmed`, refuse while `SELECT COUNT(*) FROM plugins WHERE fingerprint = ?1` is non-zero, delete, error if no key matched.
- Commands with these exact signatures:

```rust
#[tauri::command]
pub fn plugin_installed(db: tauri::State<'_, Db>) -> Result<Vec<PluginRecord>, String>
#[tauri::command]
pub fn plugin_set_enabled(db: tauri::State<'_, Db>, id: String, enabled: bool) -> Result<PluginRecord, String>
#[tauri::command]
pub fn plugin_uninstall(db: tauri::State<'_, Db>, id: String, confirmed: bool) -> Result<(), String>
#[tauri::command]
pub fn plugin_keys(db: tauri::State<'_, Db>) -> Result<Vec<TrustedKey>, String>
#[tauri::command]
pub fn plugin_remove_key(db: tauri::State<'_, Db>, fingerprint: String, confirmed: bool) -> Result<(), String>
```

Register the five commands in `lib.rs`.

- [ ] **Step 4: Run the tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: PASS, including tamper detection, downgrade refusal, key-change refusal and in-use key protection.

```bash
git add src-tauri/src/lib.rs src-tauri/src/plugins.rs
git commit -m "feat: persist and verify installed plugins"
```

---

### Task 5: Registry fetching and the install command

**Files:**
- Modify: `src-tauri/src/plugins.rs`
- Modify: `src-tauri/src/lib.rs:33-126` (register two commands)
- Test: `src-tauri/src/plugins.rs` tests

**Interfaces:**
- Produces: `http_client()`, `fetch_limited`, commands `plugin_registry(url)` and `plugin_install(entry, trustKey, confirmed)`.

- [ ] **Step 1: Write the failing network-boundary tests**

Add:

```rust
fn serve_once(response: Vec<u8>) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
        }
    });
    format!("http://{address}/plugin")
}

#[test]
fn fetch_limit_rejects_oversized_bodies() {
    let body = "x".repeat(200);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes();
    let url = reqwest::Url::parse(&serve_once(response)).unwrap();
    let result = tauri::async_runtime::block_on(async {
        let client = http_client().expect("client");
        fetch_limited(&client, &url, 100).await
    });
    assert!(result.is_err());
}

#[test]
fn client_does_not_follow_redirects() {
    let response = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let url = reqwest::Url::parse(&serve_once(response)).unwrap();
    let result = tauri::async_runtime::block_on(async {
        let client = http_client().expect("client");
        fetch_limited(&client, &url, 1024).await
    });
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests::fetch_limit_rejects_oversized_bodies plugins::tests::client_does_not_follow_redirects`
Expected: FAIL to compile because `http_client` and `fetch_limited` do not exist.

- [ ] **Step 3: Implement the HTTP client and commands**

Add constants and helpers:

```rust
const REGISTRY_MAX_BYTES: usize = 1024 * 1024;
const SIGNATURE_MAX_BYTES: usize = 1024;

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("GRAFT/0.1.0")
        .build()
        .map_err(|e| format!("cannot create the plugin HTTP client: {e}"))
}

async fn fetch_limited(
    client: &reqwest::Client,
    url: &reqwest::Url,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| format!("cannot fetch {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("The server returned HTTP {}.", response.status()));
    }
    if response.content_length().unwrap_or(0) > limit as u64 {
        return Err("The response is larger than allowed.".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("cannot read {url}: {e}"))?
    {
        if body.len() + chunk.len() > limit {
            return Err("The response is larger than allowed.".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
```

Implement:

```rust
#[tauri::command]
pub async fn plugin_registry(url: String) -> Result<PluginRegistry, String> {
    let parsed = validate_registry_url(&url)?;
    let client = http_client()?;
    let bytes = fetch_limited(&client, &parsed, REGISTRY_MAX_BYTES).await?;
    parse_registry(&bytes, parsed.as_str())
}

#[tauri::command]
pub async fn plugin_install(
    db: tauri::State<'_, Db>,
    entry: RegistryPlugin,
    trust_key: bool,
    confirmed: bool,
) -> Result<PluginRecord, String> {
    if !confirmed {
        return Err("Installation requires confirmation.".into());
    }
    let manifest_url = validate_package_url(&entry.manifest_url, true)?;
    let signature_url = validate_package_url(&entry.signature_url, true)?;
    let client = http_client()?;
    let manifest = fetch_limited(&client, &manifest_url, MANIFEST_MAX_BYTES).await?;
    let signature = fetch_limited(&client, &signature_url, SIGNATURE_MAX_BYTES).await?;
    let signature = String::from_utf8(signature)
        .map_err(|_| "The signature file is not UTF-8 text.".to_string())?;
    install_verified(&db, &entry, &manifest, signature.trim(), trust_key, confirmed)
}
```

Register both commands in `lib.rs`. `plugin_registry` is network-only and must not be called by boot code.

- [ ] **Step 4: Run the tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml plugins::tests`
Expected: PASS, including the size cap and no-redirect tests.

```bash
git add src-tauri/src/lib.rs src-tauri/src/plugins.rs
git commit -m "feat: fetch registries and install plugins"
```

---

### Task 6: IPC wrappers and architecture documentation

**Files:**
- Modify: `src/lib/api.ts:13-26`, `src/lib/api.ts:1010-1016`
- Modify: `docs/ARCHITECTURE.md:43-102`, `docs/ARCHITECTURE.md:102-230`
- Create: `docs/plugins/registry.md`
- Test: `npm run check`

**Interfaces:**
- Produces: `PluginKind`, `PluginTheme`, `PluginIntegration`, `PluginRecord`, `RegistryPlugin`, `PluginRegistry`, `TrustedKey`; seven `api.plugin*` wrappers.

- [ ] **Step 1: Add the TypeScript types**

Near the tool types in `api.ts`:

```ts
import type { ThemeTokens } from "./themes";

export type PluginKind = "theme" | "integration";

export type PluginTheme = {
  scheme: "dark" | "light";
  blurb: string;
  tokens: Omit<ThemeTokens, "scheme">;
};

export type PluginIntegration = {
  tools: string[];
  notes: string;
  docsUrl: string | null;
};

export type PluginRecord = {
  id: string;
  name: string;
  version: string;
  kind: PluginKind;
  summary: string;
  author: string;
  capabilities: string[];
  enabled: boolean;
  verified: boolean;
  fingerprint: string;
  installedAt: number;
  theme?: PluginTheme;
  integration?: PluginIntegration;
};

export type RegistryPlugin = {
  id: string;
  name: string;
  version: string;
  kind: PluginKind;
  summary: string;
  fingerprint: string;
  publicKey: string;
  manifestUrl: string;
  manifestSha256: string;
  signatureUrl: string;
};

export type PluginRegistry = {
  name: string;
  updatedAt: string;
  plugins: RegistryPlugin[];
};

export type TrustedKey = {
  fingerprint: string;
  publicKey: string;
  label: string;
  addedAt: number;
  usedBy: string[];
};
```

- [ ] **Step 2: Add the seven wrappers**

Add after the AI wrappers:

```ts
pluginInstalled: () => call<PluginRecord[]>("plugin_installed"),
pluginRegistry: (url: string) => call<PluginRegistry>("plugin_registry", { url }),
pluginInstall: (entry: RegistryPlugin, trustKey: boolean, confirmed: boolean) =>
  call<PluginRecord>("plugin_install", { entry, trustKey, confirmed }),
pluginSetEnabled: (id: string, enabled: boolean) =>
  call<PluginRecord>("plugin_set_enabled", { id, enabled }),
pluginUninstall: (id: string, confirmed: boolean) =>
  call<void>("plugin_uninstall", { id, confirmed }),
pluginKeys: () => call<TrustedKey[]>("plugin_keys"),
pluginRemoveKey: (fingerprint: string, confirmed: boolean) =>
  call<void>("plugin_remove_key", { fingerprint, confirmed }),
```

- [ ] **Step 3: Update `ARCHITECTURE.md`**

- Runtime layout: add `Plugins` to views; add `plugins`, `themeCatalog` to state.
- Change “Data model (schema 6)” to schema 7 and add the `plugins` and `plugin_keys` tables exactly as in the spec.
- Add `plugins.registryUrl` to the settings table: `string — user-supplied HTTPS registry index; empty means no registry`.
- Add seven IPC rows matching the signatures above, including that `plugin_uninstall` and `plugin_remove_key` require `confirmed`.
- Security posture: signed declarative plugins, SHA-256 before parsing, Ed25519 over exact bytes, trust-on-first-use fingerprints, no code execution, no redirects, no archive/filesystem paths.

- [ ] **Step 4: Write the registry guide**

Create `docs/plugins/registry.md` with:

- The manifest schema 1 example and field rules.
- The registry index schema 1 example and URL/size rules.
- Signing and hashing instructions using external tooling (OpenSSL 3 or Python `cryptography`), explicitly noting GRAFT does not ship a signing key.
- A statement that `manifestSha256` is lowercase hex SHA-256 of the exact manifest bytes and the signature is base64 Ed25519 over those bytes.

- [ ] **Step 5: Verify and commit**

Run: `npm run check`
Expected: PASS with 0 errors and 0 warnings.

```bash
git add src/lib/api.ts docs/ARCHITECTURE.md docs/plugins/registry.md
git commit -m "feat: expose plugin commands to the UI"
```

---

### Task 7: Plugin marketplace state

**Files:**
- Create: `src/lib/state/plugins.svelte.ts`
- Test: `npm run check`

**Interfaces:**
- Produces: `plugins` `$state`; `loadPlugins()`, `refreshRegistry()`, `saveRegistryUrl(url)`, `install(entry, trustKey)`, `setEnabled(id, enabled)`, `uninstall(id)`, `removeKey(fingerprint)`.
- `loadPlugins` is local-only and never fetches the registry.

- [ ] **Step 1: Implement the state module**

Create `src/lib/state/plugins.svelte.ts` with:

```ts
import { api, type PluginRecord, type PluginRegistry, type RegistryPlugin, type TrustedKey } from "../api";

const REGISTRY_KEY = "plugins.registryUrl";

type PluginsState = {
  loaded: boolean;
  loading: boolean;
  error: string | null;
  installed: PluginRecord[];
  keys: TrustedKey[];
  registryUrl: string;
  registry: PluginRegistry | null;
  registryLoading: boolean;
  registryError: string | null;
  busyId: string | null;
  installError: string | null;
};

export const plugins = $state<PluginsState>({
  loaded: false,
  loading: false,
  error: null,
  installed: [],
  keys: [],
  registryUrl: "",
  registry: null,
  registryLoading: false,
  registryError: null,
  busyId: null,
  installError: null,
});

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function reloadLocal(): Promise<void> {
  const [installed, keys] = await Promise.all([api.pluginInstalled(), api.pluginKeys()]);
  plugins.installed = installed;
  plugins.keys = keys;
}

export async function loadPlugins(): Promise<void> {
  if (plugins.loading) return;
  plugins.loading = true;
  plugins.error = null;
  try {
    const [installed, keys, url] = await Promise.all([
      api.pluginInstalled(),
      api.pluginKeys(),
      api.getSetting(REGISTRY_KEY),
    ]);
    plugins.installed = installed;
    plugins.keys = keys;
    plugins.registryUrl = url ?? "";
    plugins.loaded = true;
  } catch (error) {
    plugins.error = message(error);
  } finally {
    plugins.loading = false;
  }
}

export async function refreshRegistry(): Promise<void> {
  const url = plugins.registryUrl.trim();
  plugins.registryError = null;
  plugins.registry = null;
  if (!url) return;
  plugins.registryLoading = true;
  try {
    plugins.registry = await api.pluginRegistry(url);
  } catch (error) {
    plugins.registryError = message(error);
  } finally {
    plugins.registryLoading = false;
  }
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

export function saveRegistryUrl(url: string): void {
  plugins.registryUrl = url.trim();
  plugins.registry = null;
  plugins.registryError = null;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api.setSetting(REGISTRY_KEY, plugins.registryUrl).catch(() => {});
  }, 400);
}

export async function install(entry: RegistryPlugin, trustKey: boolean): Promise<PluginRecord | null> {
  plugins.busyId = entry.id;
  plugins.installError = null;
  try {
    const record = await api.pluginInstall(entry, trustKey, true);
    await reloadLocal();
    return record;
  } catch (error) {
    plugins.installError = message(error);
    return null;
  } finally {
    plugins.busyId = null;
  }
}

export async function setEnabled(id: string, enabled: boolean): Promise<void> {
  plugins.busyId = id;
  plugins.installError = null;
  try {
    await api.pluginSetEnabled(id, enabled);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  } finally {
    plugins.busyId = null;
  }
}

export async function uninstall(id: string): Promise<void> {
  plugins.busyId = id;
  plugins.installError = null;
  try {
    await api.pluginUninstall(id, true);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  } finally {
    plugins.busyId = null;
  }
}

export async function removeKey(fingerprint: string): Promise<void> {
  plugins.installError = null;
  try {
    await api.pluginRemoveKey(fingerprint, true);
    await reloadLocal();
  } catch (error) {
    plugins.installError = message(error);
  }
}
```

- [ ] **Step 2: Verify and commit**

Run: `npm run check`
Expected: PASS. (No frontend test runner exists in this repository; the state is exercised by the build and the manual pass in Task 9.)

```bash
git add src/lib/state/plugins.svelte.ts
git commit -m "feat: add plugin marketplace state"
```

---

### Task 8: Theme catalog, appearance and boot order

**Files:**
- Create: `src/lib/state/themeCatalog.svelte.ts`
- Modify: `src/lib/state/appearance.svelte.ts:1-143`
- Modify: `src/views/Settings.svelte:1-105`
- Modify: `src/lib/components/CommandPalette.svelte:1-76`
- Modify: `src/App.svelte:1-43`
- Test: `npm run check`

**Interfaces:**
- Produces: `pluginTheme(record)`, `allThemes()`, `findTheme(id)`, `reconcileThemeId(id)`, `reconcileTheme()`.
- Consumes: `plugins.installed` from Task 7.

- [ ] **Step 1: Create the dynamic catalog**

```ts
import { DEFAULT_THEME_ID, themes, type Theme } from "../themes";
import type { PluginRecord } from "../api";
import { plugins } from "./plugins.svelte";

export function pluginTheme(record: PluginRecord): Theme | null {
  if (record.kind !== "theme" || !record.theme) return null;
  return {
    id: `plugin:${record.id}`,
    name: record.name,
    blurb: record.theme.blurb,
    tokens: { scheme: record.theme.scheme, ...record.theme.tokens },
  };
}

export function allThemes(): Theme[] {
  const extra = plugins.installed
    .filter((plugin) => plugin.kind === "theme" && plugin.enabled && plugin.verified)
    .map(pluginTheme)
    .filter((theme): theme is Theme => !!theme);
  return [...themes, ...extra];
}

export function findTheme(id: string): Theme {
  return (
    allThemes().find((theme) => theme.id === id) ??
    themes.find((theme) => theme.id === DEFAULT_THEME_ID) ??
    themes[0]
  );
}

export function reconcileThemeId(id: string): string {
  return allThemes().some((theme) => theme.id === id) ? id : DEFAULT_THEME_ID;
}
```

- [ ] **Step 2: Route appearance through the catalog**

In `appearance.svelte.ts`:

- Replace the static `themeById`/`themes` imports with `allThemes`, `findTheme`, `reconcileThemeId`.
- `currentTheme()` returns `findTheme(appearance.themeId)`.
- `loadAppearance` validates `parsed.themeId` against `allThemes()`, then sets `appearance.themeId = reconcileThemeId(appearance.themeId)` before `applyAppearance()`.
- `setTheme` validates against `allThemes()`.
- `nextTheme` uses `allThemes().map((theme) => theme.id)`.
- Add:

```ts
export function reconcileTheme(): void {
  const next = reconcileThemeId(appearance.themeId);
  if (next === appearance.themeId) return;
  appearance.themeId = next;
  applyAppearance();
  persist();
}
```

- [ ] **Step 3: Update Settings and the command palette**

`Settings.svelte`: import `allThemes`; add `const themeList = $derived(allThemes());`; iterate `themeList` instead of `themes`.

`CommandPalette.svelte`: import `allThemes`; replace the static `commands` array with a `$derived` value so plugin themes appear and disappear:

```ts
const commands = $derived([
  ...NAV_ITEMS.map((item) => ({
    id: `go:${item.id}`,
    label: `Open ${item.label}`,
    group: "Navigate",
    hint: item.status === "planned" ? `Phase ${item.phase}` : "Ready",
    run: () => navigate(item.id),
  })),
  ...allThemes().map((theme) => ({
    id: `theme:${theme.id}`,
    label: `Theme: ${theme.name}`,
    group: "Appearance",
    hint: theme.blurb,
    run: () => setTheme(theme.id),
  })),
  // ...the existing density, sidebar, dashboard and nav entries unchanged
]);
```

- [ ] **Step 4: Make boot load plugins before appearance**

In `App.svelte`, import `loadPlugins` and change the boot block to:

```ts
await boot();
await loadPlugins();
await Promise.all([loadAppearance(), loadNav(), loadDashboard(), loadWatchtower()]);
await refreshTools();
```

`loadPlugins` only reads SQLite and the registry-URL setting; it must not call `refreshRegistry`.

- [ ] **Step 5: Verify and commit**

Run: `npm run check`
Expected: PASS with 0 errors and 0 warnings.

```bash
git add src/App.svelte src/lib/components/CommandPalette.svelte src/lib/state/appearance.svelte.ts src/lib/state/themeCatalog.svelte.ts src/views/Settings.svelte
git commit -m "feat: include plugin themes in the theme catalog"
```

---

### Task 9: Plugin Marketplace view, route and navigation

**Files:**
- Create: `src/views/Plugins.svelte`
- Modify: `src/App.svelte:19-21`, `src/App.svelte:140-148`
- Modify: `src/lib/nav.ts:172-184`
- Test: `npm run check`, `npm run build`

**Interfaces:**
- Consumes: all Task 7 functions, `allThemes`/`reconcileTheme`, `session.tools`, `refreshTools`.
- Produces: a routed `plugins` view with registry, installed, browse, keys and integrations sections.

- [ ] **Step 1: Build the view**

Create `src/views/Plugins.svelte` following the existing `view`/`block`/`btn`/`mono-label`/`badge` classes. It must contain:

- **Header:** “Plugin Marketplace” and the line “Secured by signatures”.
- **Registry:** text input bound to a local `registryDraft`, `Save` calls `saveRegistryUrl(registryDraft)` then `refreshRegistry()`, `Refresh` calls `refreshRegistry()`. Show `plugins.registryLoading`, `plugins.registryError` in a separate block, and `plugins.registry?.name`, `updatedAt`, plugin count when loaded.
- **Empty registry:** no sample plugins. Show a plain empty state and a `<details>` containing the registry schema 1 JSON (the webview cannot open local files).
- **Installed:** two groups, Themes and Integrations. Each row shows name, version, kind, `verified` badge, enabled state, the fingerprint, and `Disable`/`Enable` plus `Remove`. Disable/enable calls `setEnabled`; remove asks `confirm()` then calls `uninstall`; both then call `reconcileTheme()`.
- **Browse:** each registry entry shows name, version, kind and a select action. The selected entry opens a review panel with id, version, kind, summary, capabilities derived from kind (`theme.apply · applies one theme` or `integration.inspect · adds notes for detected tools`), the full fingerprint grouped in fours, and — only when `plugins.keys` does not contain the fingerprint — a checkbox “I trust this signing key”. Use `canInstall = !!selected && !plugins.busyId && (keyTrusted || trust)`: a new key needs the checkbox, while an already-trusted key can be installed by the explicit Install click. On success clear the selection/checkbox, call `reconcileTheme()`, and show `plugins.installError` on failure.
- **Trusted keys:** fingerprint, label, `usedBy` plugin ids, and Remove with `confirm()`; removal calls `removeKey`.
- **Integrations:** on mount, call `refreshTools()` when `session.tools` is empty. Render the Phase 6 set in this order: Ollama (`used by the AI workspace`), Docker (`container tooling`), GitHub CLI (`repository operations`), rust-analyzer (`language server`), Claude Code, OpenCode. For each, show the detected version or `not found`, never an invented version. Under each, show notes from enabled, verified integration plugins whose `tools` include that id; when a note has a `docsUrl`, render it as a link that calls `api.openExternal(url)`. Show a compact “other detected tools” count if the full detection list contains more.
- **Footer:** “Plugins are signed data. GRAFT never runs plugin code.”

Render every plugin-supplied string (name, summary, author, notes, blurb) with normal Svelte interpolation, never `{@html}`. Use `$derived` for grouped installed lists, the tool map, and `canInstall`; use `onclick`, not `on:click`; use only theme tokens in styles.

- [ ] **Step 2: Route and light up navigation**

In `App.svelte`, import `Plugins from "./views/Plugins.svelte"` and add `{:else if ui.view === "plugins"} <Plugins />` before the Settings branch.

In `nav.ts`, set the plugins entry to `status: "ready"` and update its summary/bullets to:

```ts
summary: "Signed declarative packages with explicit capability and key review.",
bullets: [
  "Ed25519-signed themes and integration metadata; no plugin code runs",
  "Trust-on-first-use fingerprints, version checks and readable uninstall",
  "Real detection status for Ollama, Docker, GitHub CLI, rust-analyzer, Claude Code and OpenCode",
],
```

- [ ] **Step 3: Verify and commit**

Run: `npm run check && npm run build`
Expected: both PASS; no colour literals were added.

Manual check (when a desktop session is available): install a fixture theme from a loopback registry, select it in Settings, confirm it applies, disable it in Plugins and confirm the UI falls back to `graft` and persists; uninstall it and confirm the fallback again. With the registry offline, reopen Plugins and confirm installed plugins still render while the registry error is separate.

```bash
git add src/App.svelte src/lib/nav.ts src/views/Plugins.svelte
git commit -m "feat: build the plugin marketplace view"
```

---

### Task 10: Phase documentation and final verification

**Files:**
- Modify: `docs/PLAN.md:299-305` and the current-status summary near the top
- Modify: `README.md:25-27`, `README.md:75-80`
- Test: `npm run verify`, `cargo test`

- [ ] **Step 1: Update PLAN and README**

Replace the Phase 6 placeholder with a completed section describing: signed manifests, two declarative kinds, schema 7, the registry/trust flow, the theme catalog and integrations panel, and that executable/WASM plugins remain deferred. Update the README line that calls plugins a scheduled phase and add the marketplace to the feature list. Do not claim a live public registry exists.

- [ ] **Step 2: Run the full verification**

```bash
npm run verify
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: `svelte-check` 0 errors/0 warnings; Vite build succeeds; `cargo check` succeeds; every Rust test passes. Report the actual test count.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/PLAN.md
git commit -m "docs: document the plugin marketplace"
```

## Final report requirements

- State the actual `npm run verify` and `cargo test` results.
- State explicitly that no live public registry or provider was available; registry parsing, signature verification, install rules and schema migration are unit-tested, while the loopback end-to-end UI pass is unverified unless it was actually performed.
- List any deferred minors from the final review.
