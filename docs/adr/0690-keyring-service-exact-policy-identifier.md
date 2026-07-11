# ADR 0690: Keyring Service Policy Identifier Matches Exactly

## Status

Accepted.

## Context

`secrets.keyring_service` identifies the keyring namespace used by the secret
backend. Runtime policy validation only rejected trim-empty values, and policy
loading trimmed the service name before exposing it to secret-store selection.

## Decision

`secrets.keyring_service` must be an exact non-empty value without whitespace.
Runtime policy loading preserves the configured value, and validation rejects
whitespace-bearing service names instead of normalizing them.

## Consequences

The canonical `prodex` service name remains valid. Padded keyring service names
no longer resolve to a different secret namespace through trim-normalization.
