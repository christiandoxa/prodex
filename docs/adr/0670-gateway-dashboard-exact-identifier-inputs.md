# ADR 0670: Gateway Dashboard Preserves Exact Identifier Inputs

## Status

Accepted.

## Context

The legacy gateway admin dashboard is a control-plane UI for virtual keys and
SCIM users. Server-side gateway admin APIs now reject padded key names, scope
IDs, allowed models, SCIM `userName`, and key prefixes instead of trimming them.
If the dashboard trims those fields before submitting, operators cannot see the
same exact-boundary validation that API clients receive.

## Decision

The dashboard now submits identifier and governance-scope inputs exactly as
entered. Empty optional scope inputs still become `null`. Display-oriented
strings and numeric inputs keep their existing client-side normalization.

## Consequences

Padded identifiers submitted through the dashboard fail at the same API
validation boundary as direct API requests. Canonical dashboard inputs remain
unchanged.
