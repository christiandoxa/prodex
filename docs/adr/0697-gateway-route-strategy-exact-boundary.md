# ADR 0697: Gateway Route Strategy Rejects Whitespace Padding

## Status

Accepted.

## Context

`gateway.route_aliases[].strategy` selects the model routing strategy for a
gateway route alias. Policy validation and shared runtime parsing accepted
leading and trailing whitespace before matching strategy names, so malformed
policy input could normalize into an active routing decision.

## Decision

Gateway route strategy names must be exact non-empty values without whitespace.
Policy validation and runtime strategy parsing now reject whitespace-bearing
values, and direct gateway config resolution fails closed for explicit invalid
strategies.

## Consequences

Canonical strategy names such as `fallback`, `round-robin`, and `lowest-cost`
remain valid. Padded strategy values now fail before altering route selection.
