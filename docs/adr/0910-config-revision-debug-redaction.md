# ADR 0910: Config revision debug redaction

## Status

Accepted.

## Context

Configuration revisions carry tenant IDs, revision IDs, publish timestamps, and
opaque configuration payloads. Derived `Debug` output exposed those fields to
generic diagnostics and any containing plan formatter.

## Decision

Use a custom `Debug` implementation for `ConfigRevision<T>` that redacts
tenant, revision, timestamp, and payload fields. Keep exact values available
through typed fields for validation and publication planning.

Regression coverage rejects raw tenant IDs, revision IDs, publish timestamps,
and payload strings in rendered debug output.

## Consequences

Configuration publication paths can inspect revision shape without exposing
configuration payloads or cache topology identifiers. Publication behavior
remains unchanged.
