//! Token custody. GRAFT never writes credentials to SQLite, logs or the webview
//! storage — the only home for a GitHub token is the freedesktop Secret Service.

use keyring::Entry;

const SERVICE: &str = "dev.graft.desktop";
const ACCOUNT: &str = "github";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(explain)
}

fn explain(error: keyring::Error) -> String {
    match error {
        keyring::Error::PlatformFailure(cause) => format!(
            "The system keyring is unavailable ({cause}). GRAFT keeps tokens only in the \
             Secret Service — start gnome-keyring or KWallet, then try again."
        ),
        keyring::Error::NoEntry => "No GitHub token is stored in the system keyring.".to_string(),
        other => format!("System keyring error: {other}"),
    }
}

pub fn save(token: &str) -> Result<(), String> {
    entry()?.set_password(token.trim()).map_err(explain)
}

/// `Ok(None)` means "signed out", not an error.
pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(secret) if secret.trim().is_empty() => Ok(None),
        // Credentials pasted or piped in can carry a trailing newline; a raw newline
        // inside a bearer header is invalid and surfaces as a generic builder error.
        Ok(secret) => Ok(Some(secret.trim().to_string())),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(other) => Err(explain(other)),
    }
}

/// Returns whether a credential was actually removed.
pub fn clear() -> Result<bool, String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(other) => Err(explain(other)),
    }
}
