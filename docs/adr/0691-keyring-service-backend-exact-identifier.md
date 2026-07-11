# ADR 0691: Keyring Backend Service Identifier Matches Exactly

## Status

Accepted.

## Context

`KeyringSecretBackend::new` accepts the service namespace used for keyring
locations. The constructor rejected trim-empty values but allowed
whitespace-bearing service names, leaving callers to normalize or preserve
ambiguous keyring namespaces inconsistently.

## Decision

The keyring backend service name must be an exact non-empty value without
whitespace at the secret-store boundary. `KeyringSecretBackend::new` now rejects
whitespace-bearing service names before a backend can be constructed.

## Consequences

Canonical service names such as `prodex` remain valid. Padded service names no
longer create backend instances that can accidentally target a different
keyring namespace after upstream normalization.
