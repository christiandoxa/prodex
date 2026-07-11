# ADR 0452: Postgres storage display errors are redacted

## Status

Accepted.

## Context

PostgreSQL is the durable source of truth for enterprise storage. Its response
planner already returns stable redacted failures, but local `Display` text
included tenant IDs, virtual-key IDs, schema versions, and usage amounts.

## Decision

`PostgresStoragePlanError::Display` now uses generic messages for schema-version,
tenant, virtual-key, and usage/accounting failures. Structured fields remain
available for trusted diagnostics and tests.

## Consequences

Accidental display paths no longer expose tenant identifiers, resource IDs,
schema versions, or usage amounts from Postgres storage planning.
