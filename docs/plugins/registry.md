# Plugin registry and signing

GRAFT plugins are **signed, declarative JSON packages**. A package cannot run
code, ship an archive, or name a filesystem path. This document is the external
author's reference for the manifest, the registry index and signing. The
installed-side trust model is described in
[`docs/superpowers/specs/2026-11-09-plugin-marketplace-design.md`](../superpowers/specs/2026-11-09-plugin-marketplace-design.md).

## Package format

A package is two documents:

1. **Manifest** — UTF-8 JSON, at most 64 KiB. The exact bytes are signed.
2. **Signature** — base64 text of a 64-byte Ed25519 detached signature over
   those exact manifest bytes, at most 1 KiB including whitespace.

There is no archive and no executable payload. A registry entry supplies the
public key; the user trusts a key by fingerprint on first install.

## Manifest schema 1

Unknown fields are rejected, so extra data cannot bypass validation. `schema`
must be `1`.

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

For an integration package, use `kind: "integration"`, capability
`integration.inspect`, and replace `theme` with:

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

### Field rules

| Field | Rule |
|---|---|
| `schema` | integer, exactly `1` |
| `id` | `^[a-z0-9][a-z0-9._-]{2,80}$`, no `plugin:` prefix, unique in the registry |
| `name` | 1–60 characters after trimming |
| `version` | valid SemVer, at most 32 characters |
| `kind` | `theme` or `integration` |
| `summary` | 1–200 characters after trimming |
| `author` | 1–100 characters after trimming |
| `homepage` | optional absolute HTTPS URL, no credentials or fragments, at most 300 characters |
| `minAppVersion` | optional SemVer; install refuses if newer than the running GRAFT version |
| `capabilities` | exactly `["theme.apply"]` for a theme, `["integration.inspect"]` for an integration |
| `theme` | required for a theme, forbidden for an integration |
| `integration` | required for an integration, forbidden for a theme |

Theme payload:

- `scheme`: `dark` or `light`.
- `blurb`: 1–100 characters.
- `tokens`: exactly the 17 colour names shown above — no extras, no omissions.
  `scheme` is a sibling field, not a token. Every value is at most 64
  characters and must be either `#` plus 3, 6 or 8 hex digits, or
  `rgba(r, g, b, a)` with channels 0–255 and alpha 0–1. Named colours,
  `hsl()`, `var()`, `url()` and gradients are rejected.

Integration payload:

- `tools`: 1–12 unique ids from GRAFT's fixed tool allowlist, for example
  `ollama`, `docker`, `podman`, `gh`, `rust-analyzer`, `claude`, `opencode`,
  `git`, `node`, `python3`. Unknown ids are rejected.
- `notes`: 1–2000 characters of plain text.
- `docsUrl`: optional absolute HTTPS URL. GRAFT shows it as selectable text and
  does not open it.

## Registry index schema 1

A registry is a JSON document served over HTTPS. There is no built-in
registry. The user enters the URL; plain HTTP is accepted only for loopback
hosts (`localhost`, `127.0.0.0/8`, `::1`) during development.

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
      "manifestSha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "signatureUrl": "https://example.dev/night-owl/plugin.json.sig",
      "publicKey": "base64 of 32 raw Ed25519 public-key bytes"
    }
  ]
}
```

Rules:

- At most 500 entries and at most 1 MiB for the whole index.
- Entries must have unique `id` values; duplicate ids reject the whole index.
- Registry URL and package URLs must be absolute with a host, no credentials and
  no fragments. When the registry itself is remote, `manifestUrl` and
  `signatureUrl` must be HTTPS and non-loopback; a loopback registry may point
  at HTTPS or loopback HTTP.
- `manifestSha256` is the lowercase hex SHA-256 of the exact manifest bytes,
  checked **before** the manifest is parsed.
- `publicKey` is standard base64 of exactly 32 raw Ed25519 public-key bytes.
  GRAFT computes the fingerprint itself as the lowercase hex SHA-256 of those
  bytes; an index cannot supply a fingerprint.
- The manifest's `id`, `version`, `kind`, `name` and `summary` must equal the
  index entry.
- The index is a discovery document, not a trust anchor. It is not signed;
  trust comes from the package signature and the user's key decision.
- Redirects are not followed. Timeouts are 20 s to connect and 30 s total.

The registry is fetched only when the Plugins view opens or the user presses
Refresh, never during application boot.

## Signing

Sign the **exact bytes** you publish. If you reformat the JSON or add a trailing
newline after signing, verification fails. The signature is over the raw
manifest, not a canonicalised re-serialisation.

### OpenSSL 3

```bash
# One-time key generation. Keep the private key offline.
openssl genpkey -algorithm ED25519 -out private.pem
openssl pkey -in private.pem -pubout -out public.pem

# Publish the raw 32-byte public key, base64 encoded.
openssl pkey -in private.pem -pubout -outform DER | tail -c 32 | base64 -w0
# -> put this string in the registry entry's "publicKey"

# Sign the exact manifest bytes.
openssl pkeyutl -sign -inkey private.pem -rawin -in plugin.json -out plugin.sig
base64 -w0 plugin.sig > plugin.json.sig

# Registry checksum.
sha256sum plugin.json
# -> put the hex digest in "manifestSha256"

# Optional: verify the fingerprint GRAFT will display.
openssl pkey -in private.pem -pubout -outform DER | tail -c 32 | sha256sum
```

### Python `cryptography`

```python
import base64
import hashlib
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    NoEncryption,
    PrivateFormat,
    PublicFormat,
)

manifest = Path("plugin.json").read_bytes()
key = Ed25519PrivateKey.generate()

public_raw = key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
print("publicKey:", base64.b64encode(public_raw).decode())
print("fingerprint:", hashlib.sha256(public_raw).hexdigest())
print("manifestSha256:", hashlib.sha256(manifest).hexdigest())

signature = key.sign(manifest)
Path("plugin.json.sig").write_bytes(base64.b64encode(signature))
Path("private.key").write_bytes(
    key.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())
)  # store this securely, never publish it
```

GRAFT ships no signing key and does not generate or upload keys. Publishing a
registry is entirely the author's operation.

## Local development

Serve the registry and package files from a loopback HTTP server and enter that
URL in the Plugins view. Use a fresh plugin id while testing. If GRAFT already
installed that id under another key, uninstall it first; a silent key change is
refused by design.
