# ADR 0380: Observability Secret Provider Operation Metric

## Status

Accepted.

## Context

Enterprise deployments need SecretProvider adapters for development file storage, local keyrings, and production external secret managers. Operators need to count secret read, write, delete, and revision lookup outcomes without exposing tenant IDs, secret IDs, paths, keyring accounts, credential values, key material, request IDs, or raw provider errors as metric labels.

## Decision

Add `plan_secret_provider_metric` to `prodex-observability`.

The planner emits `prodex_secret_provider_operations_total`, increments by one, and uses only the closed enum labels `secret_backend`, `secret_operation`, and `secret_result`.

## Consequences

- SecretProvider adapters can publish operation outcomes through a shared low-cardinality contract.
- Secret IDs, paths, keyring accounts, credential values, key material, tenants, request IDs, and raw provider errors remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the secret provider telemetry contract before a concrete metrics backend is wired in.
