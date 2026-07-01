# ADR 0682: Gateway Route Alias Model Identifiers Match Exactly

## Status

Accepted.

## Context

Gateway route aliases map alias names to concrete model IDs and optional
per-model metrics. These model IDs affect routing and policy decisions. Policy
validation and runtime config resolution previously trimmed route alias model
IDs, so padded values could be normalized into active routing state.

## Decision

Route alias model IDs and route metric model IDs must now be exact non-empty
values without whitespace. Metric rows must match one configured route alias
model exactly.

## Consequences

Canonical route alias model IDs are unchanged. Padded or whitespace-bearing
model IDs fail closed in policy files and are no longer trim-normalized into
active routing policy.
