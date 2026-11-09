use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tauri::State;

use crate::db::{now, Db};
use crate::tools;

const MANIFEST_MAX_BYTES: usize = 64 * 1024;
const MAX_TOKEN_CHARS: usize = 64;
const TOKEN_COUNT: usize = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Theme,
    Integration,
}

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
    pub bg: String,
    pub bg_elev: String,
    pub surface: String,
    pub surface_alt: String,
    pub border: String,
    pub border_strong: String,
    pub text: String,
    pub text_muted: String,
    pub text_faint: String,
    pub accent: String,
    pub accent_soft: String,
    pub accent_ink: String,
    pub ok: String,
    pub warn: String,
    pub danger: String,
    pub info: String,
    pub shadow: String,
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

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    if bytes.len() > MANIFEST_MAX_BYTES {
        return Err(format!(
            "The manifest is larger than {} KiB.",
            MANIFEST_MAX_BYTES / 1024
        ));
    }
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("The plugin manifest is not valid: {error}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema != 1 {
        return Err(format!(
            "Unsupported plugin manifest schema {} (expected 1).",
            manifest.schema
        ));
    }
    if !valid_id(&manifest.id) {
        return Err("The plugin id is not a valid lowercase package id.".into());
    }
    if !length_ok(&manifest.name, 1, 60) {
        return Err("The plugin name must be between 1 and 60 characters.".into());
    }
    if !length_ok(&manifest.summary, 1, 200) {
        return Err("The plugin summary must be between 1 and 200 characters.".into());
    }
    if !length_ok(&manifest.author, 1, 100) {
        return Err("The plugin author must be between 1 and 100 characters.".into());
    }
    if manifest.version.chars().count() > 32 {
        return Err("The plugin version is too long.".into());
    }
    semver::Version::parse(&manifest.version)
        .map_err(|_| "The plugin version must be valid SemVer.".to_string())?;

    if let Some(homepage) = &manifest.homepage {
        validate_https_url(homepage, "homepage")?;
    }
    if let Some(minimum) = &manifest.min_app_version {
        if minimum.chars().count() > 32 {
            return Err("minAppVersion is too long.".into());
        }
        let minimum = semver::Version::parse(minimum)
            .map_err(|_| "minAppVersion must be valid SemVer.".to_string())?;
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .map_err(|_| "The application version is not valid SemVer.".to_string())?;
        if minimum > current {
            return Err(format!(
                "This plugin needs GRAFT {minimum} or newer (this build is {current})."
            ));
        }
    }

    let expected = match manifest.kind {
        PluginKind::Theme => "theme.apply",
        PluginKind::Integration => "integration.inspect",
    };
    if manifest.capabilities.len() != 1 || manifest.capabilities[0] != expected {
        return Err(format!(
            "The capabilities must be exactly [\"{expected}\"] for this plugin kind."
        ));
    }

    match manifest.kind {
        PluginKind::Theme => {
            if manifest.integration.is_some() {
                return Err("A theme plugin cannot carry integration metadata.".into());
            }
            let theme = manifest
                .theme
                .as_ref()
                .ok_or_else(|| "A theme plugin must include a theme payload.".to_string())?;
            validate_theme(theme)?;
        }
        PluginKind::Integration => {
            if manifest.theme.is_some() {
                return Err("An integration plugin cannot carry a theme payload.".into());
            }
            let integration = manifest.integration.as_ref().ok_or_else(|| {
                "An integration plugin must include integration metadata.".to_string()
            })?;
            validate_integration(integration)?;
        }
    }

    Ok(())
}

fn validate_theme(theme: &ThemePayload) -> Result<(), String> {
    if theme.scheme != "dark" && theme.scheme != "light" {
        return Err("The theme scheme must be dark or light.".into());
    }
    if !length_ok(&theme.blurb, 1, 100) {
        return Err("The theme blurb must be between 1 and 100 characters.".into());
    }

    let tokens: [&String; TOKEN_COUNT] = [
        &theme.tokens.bg,
        &theme.tokens.bg_elev,
        &theme.tokens.surface,
        &theme.tokens.surface_alt,
        &theme.tokens.border,
        &theme.tokens.border_strong,
        &theme.tokens.text,
        &theme.tokens.text_muted,
        &theme.tokens.text_faint,
        &theme.tokens.accent,
        &theme.tokens.accent_soft,
        &theme.tokens.accent_ink,
        &theme.tokens.ok,
        &theme.tokens.warn,
        &theme.tokens.danger,
        &theme.tokens.info,
        &theme.tokens.shadow,
    ];
    for (index, token) in tokens.iter().enumerate() {
        if token.chars().count() > MAX_TOKEN_CHARS || !valid_colour(token) {
            return Err(format!("Theme token {index} is not a valid colour."));
        }
    }
    Ok(())
}

fn validate_integration(integration: &IntegrationPayload) -> Result<(), String> {
    if integration.tools.is_empty() || integration.tools.len() > 12 {
        return Err("An integration must name between 1 and 12 tools.".into());
    }
    let mut seen = std::collections::HashSet::new();
    for tool in &integration.tools {
        if !tools::is_known_tool(tool) {
            return Err(format!("Unknown integration tool: {tool}."));
        }
        if !seen.insert(tool.as_str()) {
            return Err(format!("Duplicate integration tool: {tool}."));
        }
    }
    if !length_ok(&integration.notes, 1, 2000) {
        return Err("Integration notes must be between 1 and 2000 characters.".into());
    }
    if let Some(docs_url) = &integration.docs_url {
        validate_https_url(docs_url, "docsUrl")?;
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 3 || bytes.len() > 81 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    })
}

fn length_ok(value: &str, min: usize, max: usize) -> bool {
    let length = value.trim().chars().count();
    (min..=max).contains(&length)
}

fn validate_https_url(value: &str, field: &str) -> Result<(), String> {
    if value.chars().count() > 300 {
        return Err(format!("The {field} URL is too long."));
    }
    let url =
        reqwest::Url::parse(value).map_err(|_| format!("The {field} must be an absolute URL."))?;
    if url.scheme() != "https" {
        return Err(format!("The {field} must use HTTPS."));
    }
    if url.host_str().is_none() {
        return Err(format!("The {field} must include a host."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!("The {field} must not include credentials."));
    }
    if url.fragment().is_some() {
        return Err(format!("The {field} must not include a fragment."));
    }
    Ok(())
}

pub fn valid_colour(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_TOKEN_CHARS {
        return false;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }

    let lower = value.to_ascii_lowercase();
    let Some(inner) = lower
        .strip_prefix("rgba(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return false;
    }
    for part in &parts[..3] {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        let Ok(channel) = part.parse::<u16>() else {
            return false;
        };
        if channel > 255 {
            return false;
        }
    }

    let alpha = parts[3];
    if alpha.is_empty()
        || !alpha.chars().all(|c| c.is_ascii_digit() || c == '.')
        || alpha.matches('.').count() > 1
    {
        return false;
    }
    let Ok(alpha) = alpha.parse::<f64>() else {
        return false;
    };
    alpha.is_finite() && (0.0..=1.0).contains(&alpha)
}

const REGISTRY_MAX_BYTES: usize = 1024 * 1024;
const REGISTRY_MAX_ENTRIES: usize = 500;
const SIGNATURE_MAX_BYTES: usize = 1024;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryEntry {
    id: String,
    name: String,
    version: String,
    kind: PluginKind,
    summary: String,
    manifest_url: String,
    manifest_sha256: String,
    signature_url: String,
    public_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryIndex {
    schema: u32,
    name: String,
    updated_at: String,
    plugins: Vec<RegistryEntry>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

pub fn fingerprint(public_key: &[u8]) -> String {
    sha256_hex(public_key)
}

fn decode_public_key(value: &str) -> Result<[u8; 32], String> {
    let value = value.trim();
    if value.len() > 256 {
        return Err("The plugin public key is too long.".into());
    }
    let decoded = STANDARD
        .decode(value)
        .map_err(|_| "The plugin public key is not valid base64.".to_string())?;
    decoded
        .try_into()
        .map_err(|_| "The plugin public key must be 32 bytes.".to_string())
}

pub fn verify_manifest_signature(
    manifest: &[u8],
    signature_b64: &str,
    public_key_b64: &str,
) -> Result<(), String> {
    let key_bytes = decode_public_key(public_key_b64)?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| "The plugin public key is invalid.".to_string())?;
    let signature_bytes = STANDARD
        .decode(signature_b64.trim())
        .map_err(|_| "The plugin signature is not valid base64.".to_string())?;
    let signature_bytes: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| "The plugin signature must be 64 bytes.".to_string())?;
    let signature = Signature::from_bytes(&signature_bytes);
    key.verify_strict(manifest, &signature)
        .map_err(|_| "The plugin signature is invalid.".to_string())
}

fn parse_plugin_url(value: &str, allow_loopback_http: bool) -> Result<reqwest::Url, String> {
    if value.chars().count() > 2048 {
        return Err("The plugin URL is too long.".into());
    }
    let url =
        reqwest::Url::parse(value).map_err(|_| "The plugin URL must be absolute.".to_string())?;
    if url.host_str().is_none() {
        return Err("The plugin URL must include a host.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("The plugin URL must not include credentials.".into());
    }
    if url.fragment().is_some() {
        return Err("The plugin URL must not include a fragment.".into());
    }
    if !allow_loopback_http && is_loopback_url(&url) {
        return Err("A remote registry cannot point at a loopback address.".into());
    }
    if url.scheme() != "https"
        && !(url.scheme() == "http" && allow_loopback_http && is_loopback_url(&url))
    {
        return Err(
            "Plugin URLs must use HTTPS (HTTP is only allowed for loopback development).".into(),
        );
    }
    Ok(url)
}

pub fn validate_registry_url(value: &str) -> Result<reqwest::Url, String> {
    parse_plugin_url(value, true)
}

pub fn validate_package_url(
    value: &str,
    allow_loopback_http: bool,
) -> Result<reqwest::Url, String> {
    parse_plugin_url(value, allow_loopback_http)
}

pub fn is_loopback_url(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
}

pub fn parse_registry(bytes: &[u8], registry_url: &str) -> Result<PluginRegistry, String> {
    if bytes.len() > REGISTRY_MAX_BYTES {
        return Err(format!(
            "The plugin registry is larger than {} KiB.",
            REGISTRY_MAX_BYTES / 1024
        ));
    }
    let index: RegistryIndex = serde_json::from_slice(bytes)
        .map_err(|error| format!("The plugin registry is not valid: {error}"))?;
    if index.schema != 1 {
        return Err(format!(
            "Unsupported plugin registry schema {} (expected 1).",
            index.schema
        ));
    }
    if !length_ok(&index.name, 1, 100) {
        return Err("The registry name must be between 1 and 100 characters.".into());
    }
    let updated_at = index.updated_at.trim();
    if updated_at.is_empty() || updated_at.chars().count() > 40 {
        return Err("The registry updatedAt value must be between 1 and 40 characters.".into());
    }
    if index.plugins.len() > REGISTRY_MAX_ENTRIES {
        return Err(format!(
            "The plugin registry has more than {REGISTRY_MAX_ENTRIES} entries."
        ));
    }

    let registry = validate_registry_url(registry_url)?;
    let allow_loopback = is_loopback_url(&registry);
    let mut seen = std::collections::HashSet::new();
    let mut plugins = Vec::with_capacity(index.plugins.len());
    for entry in index.plugins {
        if !valid_id(&entry.id) {
            return Err(format!("The registry entry id {:?} is invalid.", entry.id));
        }
        if !seen.insert(entry.id.clone()) {
            return Err(format!(
                "The registry lists the plugin id {} more than once.",
                entry.id
            ));
        }
        if !length_ok(&entry.name, 1, 60) {
            return Err(format!(
                "The registry entry {} has an invalid name.",
                entry.id
            ));
        }
        if entry.version.chars().count() > 32 {
            return Err(format!(
                "The registry entry {} has a version that is too long.",
                entry.id
            ));
        }
        semver::Version::parse(&entry.version).map_err(|_| {
            format!(
                "The registry entry {} must have a valid SemVer version.",
                entry.id
            )
        })?;
        if !length_ok(&entry.summary, 1, 200) {
            return Err(format!(
                "The registry entry {} has an invalid summary.",
                entry.id
            ));
        }
        validate_package_url(&entry.manifest_url, allow_loopback).map_err(|error| {
            format!(
                "The registry entry {} has an invalid manifest URL: {error}",
                entry.id
            )
        })?;
        validate_package_url(&entry.signature_url, allow_loopback).map_err(|error| {
            format!(
                "The registry entry {} has an invalid signature URL: {error}",
                entry.id
            )
        })?;

        let manifest_sha256 = entry.manifest_sha256.trim().to_ascii_lowercase();
        if manifest_sha256.len() != 64
            || !manifest_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(format!(
                "The registry entry {} has an invalid manifest SHA-256.",
                entry.id
            ));
        }
        let public_key = entry.public_key.trim().to_string();
        let key = decode_public_key(&public_key).map_err(|error| {
            format!(
                "The registry entry {} has an invalid key: {error}",
                entry.id
            )
        })?;
        let fingerprint = fingerprint(&key);

        plugins.push(RegistryPlugin {
            id: entry.id,
            name: entry.name,
            version: entry.version,
            kind: entry.kind,
            summary: entry.summary,
            manifest_url: entry.manifest_url,
            manifest_sha256,
            signature_url: entry.signature_url,
            public_key,
            fingerprint,
        });
    }

    Ok(PluginRegistry {
        name: index.name,
        updated_at: index.updated_at,
        plugins,
    })
}

pub fn check_entry_matches(manifest: &Manifest, entry: &RegistryPlugin) -> Result<(), String> {
    if manifest.id != entry.id
        || manifest.version != entry.version
        || manifest.kind != entry.kind
        || manifest.name != entry.name
        || manifest.summary != entry.summary
    {
        return Err("The registry entry does not match the signed manifest.".into());
    }
    Ok(())
}

pub fn check_version(
    existing_version: Option<&str>,
    existing_fingerprint: Option<&str>,
    version: &str,
    fingerprint: &str,
) -> Result<(), String> {
    let next = semver::Version::parse(version)
        .map_err(|_| "The plugin version must be valid SemVer.".to_string())?;
    let Some(existing_version) = existing_version else {
        return Ok(());
    };
    let Some(existing_fingerprint) = existing_fingerprint else {
        return Err("The installed plugin has no recorded signing key.".into());
    };
    if !existing_fingerprint.eq_ignore_ascii_case(fingerprint) {
        return Err("This plugin id is already signed by a different key. Uninstall it before trusting a new publisher.".into());
    }
    let current = semver::Version::parse(existing_version)
        .map_err(|_| "The installed plugin version is not valid SemVer.".to_string())?;
    if next < current {
        return Err(format!(
            "This plugin version ({next}) is older than the installed version ({current})."
        ));
    }
    Ok(())
}

pub fn check_min_app_version(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.chars().count() > 32 {
        return Err("minAppVersion is too long.".into());
    }
    let minimum = semver::Version::parse(value)
        .map_err(|_| "minAppVersion must be valid SemVer.".to_string())?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| "The application version is not valid SemVer.".to_string())?;
    if minimum > current {
        return Err(format!(
            "This plugin needs GRAFT {minimum} or newer (this build is {current})."
        ));
    }
    Ok(())
}

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

#[derive(Debug, Clone)]
struct StoredPlugin {
    id: String,
    name: String,
    version: String,
    kind: String,
    summary: String,
    author: String,
    manifest: String,
    signature: String,
    public_key: String,
    fingerprint: String,
    enabled: bool,
    installed_at: i64,
}

impl StoredPlugin {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let enabled: i64 = row.get(10)?;
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            version: row.get(2)?,
            kind: row.get(3)?,
            summary: row.get(4)?,
            author: row.get(5)?,
            manifest: row.get(6)?,
            signature: row.get(7)?,
            public_key: row.get(8)?,
            fingerprint: row.get(9)?,
            enabled: enabled != 0,
            installed_at: row.get(11)?,
        })
    }
}

fn conn(db: &Db) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    db.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn kind_str(kind: PluginKind) -> &'static str {
    match kind {
        PluginKind::Theme => "theme",
        PluginKind::Integration => "integration",
    }
}

fn kind_from_str(value: &str) -> Result<PluginKind, String> {
    match value {
        "theme" => Ok(PluginKind::Theme),
        "integration" => Ok(PluginKind::Integration),
        _ => Err(format!("Unknown plugin kind: {value}.")),
    }
}

fn capabilities_for(kind: PluginKind) -> Vec<String> {
    vec![match kind {
        PluginKind::Theme => "theme.apply",
        PluginKind::Integration => "integration.inspect",
    }
    .to_string()]
}

fn stored_plugin(db: &Db, id: &str) -> Result<StoredPlugin, String> {
    let conn = conn(db);
    conn.query_row(
        "SELECT id, name, version, kind, summary, author, manifest, signature,
                public_key, fingerprint, enabled, installed_at
         FROM plugins WHERE id = ?1",
        params![id],
        StoredPlugin::from_row,
    )
    .optional()
    .map_err(|error| format!("cannot read the plugin: {error}"))?
    .ok_or_else(|| "Plugin not found.".to_string())
}

fn is_trusted(db: &Db, fingerprint: &str) -> Result<bool, String> {
    let count: i64 = conn(db)
        .query_row(
            "SELECT COUNT(*) FROM plugin_keys WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )
        .map_err(|error| format!("cannot read plugin trust: {error}"))?;
    Ok(count > 0)
}

fn read_keys(db: &Db) -> Result<Vec<TrustedKey>, String> {
    let conn = conn(db);
    let mut used_by: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT fingerprint, id FROM plugins ORDER BY id ASC")
            .map_err(|error| format!("cannot query plugin key usage: {error}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("cannot read plugin key usage: {error}"))?;
        for row in rows {
            let (fingerprint, id) =
                row.map_err(|error| format!("cannot decode plugin key usage: {error}"))?;
            used_by.entry(fingerprint).or_default().push(id);
        }
    }

    let mut stmt = conn
        .prepare(
            "SELECT fingerprint, public_key, label, added_at
             FROM plugin_keys ORDER BY added_at ASC, fingerprint ASC",
        )
        .map_err(|error| format!("cannot query trusted plugin keys: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TrustedKey {
                fingerprint: row.get(0)?,
                public_key: row.get(1)?,
                label: row.get(2)?,
                added_at: row.get(3)?,
                used_by: Vec::new(),
            })
        })
        .map_err(|error| format!("cannot read trusted plugin keys: {error}"))?;
    let mut keys = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot decode trusted plugin keys: {error}"))?;
    for key in &mut keys {
        key.used_by = used_by.remove(&key.fingerprint).unwrap_or_default();
    }
    Ok(keys)
}

fn verify_row(row: &StoredPlugin, trusted: bool) -> Result<Manifest, String> {
    if !trusted {
        return Err("The plugin signing key is not trusted.".into());
    }
    let key = decode_public_key(&row.public_key)?;
    if !fingerprint(&key).eq_ignore_ascii_case(&row.fingerprint) {
        return Err("The stored plugin key does not match its fingerprint.".into());
    }
    let manifest = parse_manifest(row.manifest.as_bytes())?;
    verify_manifest_signature(row.manifest.as_bytes(), &row.signature, &row.public_key)?;
    if manifest.id != row.id
        || manifest.name != row.name
        || manifest.version != row.version
        || manifest.kind != kind_from_str(&row.kind)?
        || manifest.summary != row.summary
        || manifest.author != row.author
    {
        return Err("The stored plugin metadata does not match its signed manifest.".into());
    }
    Ok(manifest)
}

fn record_from_row(row: &StoredPlugin, trusted: bool) -> PluginRecord {
    let verified = verify_row(row, trusted);
    let valid = verified.is_ok();
    let (kind, capabilities, theme, integration) = match verified {
        Ok(manifest) => (
            manifest.kind,
            manifest.capabilities,
            manifest.theme,
            manifest.integration,
        ),
        Err(_) => {
            let kind = kind_from_str(&row.kind);
            let capabilities = kind
                .as_ref()
                .map(|kind| capabilities_for(*kind))
                .unwrap_or_default();
            (kind.unwrap_or(PluginKind::Theme), capabilities, None, None)
        }
    };
    PluginRecord {
        id: row.id.clone(),
        name: row.name.clone(),
        version: row.version.clone(),
        kind,
        summary: row.summary.clone(),
        author: row.author.clone(),
        capabilities,
        enabled: row.enabled,
        verified: valid,
        fingerprint: row.fingerprint.clone(),
        installed_at: row.installed_at,
        theme,
        integration,
    }
}

fn read_installed(db: &Db) -> Result<Vec<PluginRecord>, String> {
    let conn = conn(db);
    let trusted: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT fingerprint FROM plugin_keys")
            .map_err(|error| format!("cannot query plugin trust: {error}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("cannot read plugin trust: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("cannot decode plugin trust: {error}"))?
            .into_iter()
            .collect()
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, name, version, kind, summary, author, manifest, signature,
                    public_key, fingerprint, enabled, installed_at
             FROM plugins ORDER BY installed_at DESC, id ASC",
        )
        .map_err(|error| format!("cannot query installed plugins: {error}"))?;
    let rows = stmt
        .query_map([], StoredPlugin::from_row)
        .map_err(|error| format!("cannot read installed plugins: {error}"))?;
    let mut records = Vec::new();
    for row in rows {
        let row = row.map_err(|error| format!("cannot decode installed plugin: {error}"))?;
        records.push(record_from_row(&row, trusted.contains(&row.fingerprint)));
    }
    Ok(records)
}

fn read_plugin(db: &Db, id: &str) -> Result<PluginRecord, String> {
    read_installed(db)?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| "Plugin not found.".to_string())
}

pub fn install_verified(
    db: &Db,
    entry: &RegistryPlugin,
    manifest_bytes: &[u8],
    signature_b64: &str,
    trust_key: bool,
    confirmed: bool,
) -> Result<PluginRecord, String> {
    if !confirmed {
        return Err("Installing a plugin requires explicit confirmation.".into());
    }
    if !sha256_hex(manifest_bytes).eq_ignore_ascii_case(entry.manifest_sha256.trim()) {
        return Err("The plugin manifest does not match the registry hash.".into());
    }

    let manifest = parse_manifest(manifest_bytes)?;
    check_entry_matches(&manifest, entry)?;
    check_min_app_version(manifest.min_app_version.as_deref())?;

    let key = decode_public_key(&entry.public_key)?;
    let key_fingerprint = fingerprint(&key);
    if !key_fingerprint.eq_ignore_ascii_case(entry.fingerprint.trim()) {
        return Err("The plugin signing key does not match the registry fingerprint.".into());
    }
    verify_manifest_signature(manifest_bytes, signature_b64, &entry.public_key)?;

    let manifest_text = std::str::from_utf8(manifest_bytes)
        .map_err(|_| "The manifest is not UTF-8.".to_string())?;
    let public_key = entry.public_key.trim().to_string();
    let label: String = manifest.author.trim().chars().take(100).collect();
    let now_value = now();

    {
        let mut connection = conn(db);
        let tx = connection
            .transaction()
            .map_err(|error| format!("cannot start the plugin install: {error}"))?;

        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT version, fingerprint FROM plugins WHERE id = ?1",
                params![manifest.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("cannot read the installed plugin: {error}"))?;
        check_version(
            existing.as_ref().map(|(version, _)| version.as_str()),
            existing
                .as_ref()
                .map(|(_, fingerprint)| fingerprint.as_str()),
            &manifest.version,
            &key_fingerprint,
        )?;

        let trusted: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM plugin_keys WHERE fingerprint = ?1",
                params![key_fingerprint],
                |row| row.get(0),
            )
            .map_err(|error| format!("cannot read plugin trust: {error}"))?;
        if trusted == 0 && !trust_key {
            return Err(
                "This plugin uses a new signing key. Review the fingerprint and trust the publisher before installing."
                    .into(),
            );
        }

        tx.execute(
            "INSERT OR IGNORE INTO plugin_keys (fingerprint, public_key, label, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![key_fingerprint, public_key, label, now_value],
        )
        .map_err(|error| format!("cannot save the trusted plugin key: {error}"))?;

        tx.execute(
            "INSERT INTO plugins
                (id, name, version, kind, summary, author, manifest, signature,
                 public_key, fingerprint, enabled, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                version = excluded.version,
                kind = excluded.kind,
                summary = excluded.summary,
                author = excluded.author,
                manifest = excluded.manifest,
                signature = excluded.signature,
                public_key = excluded.public_key,
                fingerprint = excluded.fingerprint",
            params![
                manifest.id.as_str(),
                manifest.name.as_str(),
                manifest.version.as_str(),
                kind_str(manifest.kind),
                manifest.summary.as_str(),
                manifest.author.as_str(),
                manifest_text,
                signature_b64.trim(),
                public_key.as_str(),
                key_fingerprint.as_str(),
                now_value
            ],
        )
        .map_err(|error| format!("cannot save the plugin: {error}"))?;
        tx.commit()
            .map_err(|error| format!("cannot finish the plugin install: {error}"))?;
    }

    read_plugin(db, &manifest.id)
}

pub fn set_plugin_enabled(db: &Db, id: &str, enabled: bool) -> Result<PluginRecord, String> {
    let row = stored_plugin(db, id)?;
    if enabled {
        verify_row(&row, is_trusted(db, &row.fingerprint)?)?;
    }
    let changed = conn(db)
        .execute(
            "UPDATE plugins SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1i64 } else { 0i64 }, id],
        )
        .map_err(|error| format!("cannot update the plugin: {error}"))?;
    if changed == 0 {
        return Err("Plugin not found.".into());
    }
    read_plugin(db, id)
}

pub fn uninstall_plugin(db: &Db, id: &str, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("Uninstalling a plugin requires explicit confirmation.".into());
    }
    let changed = conn(db)
        .execute("DELETE FROM plugins WHERE id = ?1", params![id])
        .map_err(|error| format!("cannot remove the plugin: {error}"))?;
    if changed == 0 {
        return Err("Plugin not found.".into());
    }
    Ok(())
}

pub fn remove_trusted_key(db: &Db, fingerprint: &str, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("Removing a trusted key requires explicit confirmation.".into());
    }
    let fingerprint = fingerprint.trim().to_ascii_lowercase();
    let in_use: i64 = conn(db)
        .query_row(
            "SELECT COUNT(*) FROM plugins WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )
        .map_err(|error| format!("cannot check plugin key usage: {error}"))?;
    if in_use > 0 {
        return Err("This key still signs installed plugins. Uninstall them first.".into());
    }
    let changed = conn(db)
        .execute(
            "DELETE FROM plugin_keys WHERE fingerprint = ?1",
            params![fingerprint],
        )
        .map_err(|error| format!("cannot remove the trusted key: {error}"))?;
    if changed == 0 {
        return Err("Trusted key not found.".into());
    }
    Ok(())
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("GRAFT/0.1.0")
        .build()
        .map_err(|error| format!("cannot create the plugin HTTP client: {error}"))
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
        .map_err(|error| format!("cannot fetch {url}: {error}"))?;
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
        .map_err(|error| format!("cannot read {url}: {error}"))?
    {
        if body.len() + chunk.len() > limit {
            return Err("The response is larger than allowed.".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[tauri::command]
pub async fn plugin_registry(url: String) -> Result<PluginRegistry, String> {
    let parsed = validate_registry_url(&url)?;
    let client = http_client()?;
    let bytes = fetch_limited(&client, &parsed, REGISTRY_MAX_BYTES).await?;
    parse_registry(&bytes, parsed.as_str())
}

#[tauri::command]
pub async fn plugin_install(
    db: State<'_, Db>,
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
    install_verified(
        &db,
        &entry,
        &manifest,
        signature.trim(),
        trust_key,
        confirmed,
    )
}

#[tauri::command]
pub fn plugin_installed(db: State<'_, Db>) -> Result<Vec<PluginRecord>, String> {
    read_installed(&db)
}

#[tauri::command]
pub fn plugin_set_enabled(
    db: State<'_, Db>,
    id: String,
    enabled: bool,
) -> Result<PluginRecord, String> {
    set_plugin_enabled(&db, &id, enabled)
}

#[tauri::command]
pub fn plugin_uninstall(db: State<'_, Db>, id: String, confirmed: bool) -> Result<(), String> {
    uninstall_plugin(&db, &id, confirmed)
}

#[tauri::command]
pub fn plugin_keys(db: State<'_, Db>) -> Result<Vec<TrustedKey>, String> {
    read_keys(&db)
}

#[tauri::command]
pub fn plugin_remove_key(
    db: State<'_, Db>,
    fingerprint: String,
    confirmed: bool,
) -> Result<(), String> {
    remove_trusted_key(&db, &fingerprint, confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
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
        value["theme"]["tokens"]
            .as_object_mut()
            .unwrap()
            .remove("shadow");
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
        assert!(
            verify_manifest_signature(manifest, &format!(" {signature} "), &public_key).is_ok()
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
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
        let remote =
            serde_json::to_vec(&registry_value("http://127.0.0.1:8000/plugin.json")).unwrap();
        assert!(parse_registry(&remote, "https://example.com/index.json").is_err());

        let loopback =
            serde_json::to_vec(&registry_value("http://127.0.0.1:8000/plugin.json")).unwrap();
        let parsed = parse_registry(&loopback, "http://127.0.0.1:9000/index.json")
            .expect("loopback registry");
        assert_eq!(parsed.plugins[0].fingerprint, fingerprint(&[9u8; 32]));
    }

    #[test]
    fn rejects_duplicate_ids_and_bad_registry_entries() {
        let mut value = registry_value("https://example.dev/plugin.json");
        let duplicate = value["plugins"][0].clone();
        value["plugins"].as_array_mut().unwrap().push(duplicate);
        assert!(parse_registry(
            &serde_json::to_vec(&value).unwrap(),
            "https://example.com/index.json"
        )
        .is_err());

        let mut value = registry_value("https://example.dev/plugin.json");
        value["plugins"][0]["publicKey"] = json!("short");
        assert!(parse_registry(
            &serde_json::to_vec(&value).unwrap(),
            "https://example.com/index.json"
        )
        .is_err());

        let mut value = registry_value("https://example.dev/plugin.json");
        value["plugins"][0]["manifestSha256"] = json!("not-hex");
        assert!(parse_registry(
            &serde_json::to_vec(&value).unwrap(),
            "https://example.com/index.json"
        )
        .is_err());
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

        assert!(check_min_app_version(Some("0.1.0")).is_ok());
        assert!(check_min_app_version(Some("999.0.0")).is_err());
    }

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

        let record =
            install_verified(&db, &entry, &manifest, &signature, true, true).expect("install");
        assert!(record.verified && record.enabled);
        assert_eq!(
            record.fingerprint,
            fingerprint(&key.verifying_key().to_bytes())
        );
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
        install_verified(
            &db,
            &entry_for(&manifest, &public_key),
            &manifest,
            &signature,
            true,
            true,
        )
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
            .execute(
                "UPDATE plugins SET signature = 'AAAA' WHERE id = ?1",
                [&record.id],
            )
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
}
