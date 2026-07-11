# ADR 0453: SQLite storage display errors are redacted

## Status

Accepted.

## Context

SQLite remains useful for development and compatibility paths, but regulated
deployments should not see adapter-specific `Display` output leak tenant IDs,
virtual-key IDs, schema versions, or usage amounts.

## Decision

`SqliteStoragePlanError::Display` now uses generic messages for schema-version,
tenant, virtual-key, and usage/accounting failures. Structured fields remain
available for trusted diagnostics and tests.

## Consequences

SQLite storage planning now follows the same redacted display contract as the
Postgres adapter while keeping runtime behavior unchanged.
