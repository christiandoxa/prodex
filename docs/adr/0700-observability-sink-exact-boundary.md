# ADR 0700: Observability Sink Names Reject Whitespace Padding

## Status

Accepted.

## Context

`gateway.observability.sinks` selects enabled gateway observability sinks such
as `runtime-log`, `jsonl`, and `http`. Policy validation only rejected
trim-empty values, and direct runtime config resolution trimmed sink names
before deciding which exporters were active.

## Decision

Observability sink names must be exact non-empty values without whitespace.
Policy validation rejects whitespace-bearing sink names, and direct runtime
config resolution fails closed instead of trim-normalizing or silently dropping
padded sink selectors.

## Consequences

Canonical sink names remain valid, and automatic `runtime-log`, `jsonl`, and
`http` enablement still works. Padded sink names no longer silently activate,
disable, or alter observability exporters.
