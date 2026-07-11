# ADR 0693: Keyring Location Account Must Be Non-Empty

## Status

Accepted.

## Context

Keyring secret locations use a service namespace and account identifier. The
backend validated that the location service matched the configured backend
service, but it accepted an empty account identifier.

## Decision

`KeyringSecretBackend` rejects keyring locations with an empty account before
read, write, delete, or revision probing proceeds.

## Consequences

Deterministic account identifiers such as `auth-json:<codex-home>` remain
valid. Empty keyring accounts now fail closed at the secret-store boundary
instead of reaching backend-specific handling.
