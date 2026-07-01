# ADR 0922: Storage topology debug redaction

## Status

Accepted.

## Context

`StorageTopology` carries durable-store kind, cache-store kind, and migration
runtime policy. The fields are needed for planning, but derived debug output
exposed exact PostgreSQL, Redis, SQLite, and migration-mode topology.

## Decision

Use a custom `Debug` implementation for `StorageTopology` that redacts store and
migration-policy fields. Keep typed fields public for planner decisions and
tests.

Regression coverage rejects store names and migration policy names in rendered
topology debug output.

## Consequences

Storage topology remains available as structured data without leaking deployment
shape through generic debug formatting.
