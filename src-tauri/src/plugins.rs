use serde::{Deserialize, Serialize};

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
}
