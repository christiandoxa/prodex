# ADR 0705: Gateway Observability JSONL Path Exact Boundary

## Status

Accepted.

## Context

`gateway.observability.jsonl_path` is a filesystem path for JSONL telemetry
export. Runtime launch previously trimmed the configured path before resolving
it under the Prodex root. That silently changed valid filesystem names with
leading or trailing spaces.

## Decision

Preserve non-blank `gateway.observability.jsonl_path` values exactly at runtime.
Blank-only values remain invalid through policy validation and are ignored by
the runtime helper.

## Consequences

Operators get the exact path they configured. Accidental padding in path values
now points at the padded filename instead of a silently normalized filename, so
configuration review can catch it.
