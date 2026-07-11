# ADR 0985: Secret Store Debug Redaction

## Status

Accepted

## Context

`prodex-secret-store` already renders stable redacted display and response
messages, but derived debug output for secret locations, values, and errors
could still expose filesystem paths, keyring accounts, credential bytes, or
backend error details in trusted-but-copyable diagnostics.

## Decision

Use custom `Debug` implementations for `SecretLocation`, `SecretValue`,
`SecretError`, `KeyringSecretBackend`, `SecretBackendSelection`, and refresh
lease coordination types. `SecretRevision` debug output also redacts size and
modified-at metadata. Preserve variant names for diagnostics, but redact paths,
locations, reasons, keyring service/account names, refresh result JSON, lease
digests, timing values, and secret bytes.

## Consequences

- Accidental debug logging no longer exposes secret-store paths, credential
  material, refresh-lease result payloads, or secret revision metadata.
- Trusted diagnostics that need raw fields must access typed fields directly
  instead of relying on derived debug output.
