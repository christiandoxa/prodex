# ADR 0696: Gateway State Backend Rejects Whitespace Padding

## Status

Accepted.

## Context

`gateway.state.backend` selects the gateway state storage backend. Policy
validation and direct runtime config resolution accepted leading and trailing
whitespace before matching backend names, so malformed policy input could
normalize into an active file, SQLite, PostgreSQL, or Redis backend.

## Decision

Gateway state backend names must be exact non-empty values without whitespace.
Policy validation rejects whitespace-bearing backend names, and direct runtime
config resolution fails closed instead of trimming or falling back.

## Consequences

Canonical backend names `file`, `sqlite`, `postgres`, and `redis` remain valid.
Padded backend values now fail before activating a durable state backend choice.
