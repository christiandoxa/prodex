# ADR 0596: Gateway HTTP Request Target Route Normalization

## Status

Accepted.

## Context

Gateway HTTP route and API-version planners may receive either a framework path
or a raw request target. Raw request targets include query strings or fragments.
Matching those strings directly can misclassify valid data-plane or
control-plane routes as unknown.

## Decision

The gateway HTTP boundary strips query and fragment suffixes before route
classification, control-plane operation planning, and API-version extraction.

## Consequences

Adapters can pass normalized paths or raw request targets without changing
authorization, idempotency, audit, or route semantics. Query parsing remains
owned by the pagination/query helpers.
