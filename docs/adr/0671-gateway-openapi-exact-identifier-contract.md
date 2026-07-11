# ADR 0671: Gateway OpenAPI Exact Identifier Contract

## Status

Accepted.

## Context

Gateway admin APIs reject padded or whitespace-bearing key names, model names,
SCIM principals, key prefixes, and scope IDs. The OpenAPI document still
described those fields as unconstrained strings, so generated clients and
operators could miss the exact-boundary contract.

## Decision

The gateway OpenAPI components now expose reusable exact identifier schemas:

- `GatewayExactIdentifier`: non-empty string without whitespace.
- `GatewayNullableExactIdentifier`: exact identifier or `null`.

Key create/patch/read schemas and SCIM user write/read schemas reference these
components for identifier and governance-scope fields.

## Consequences

The documented API contract now matches the server-side validation boundary.
Generated clients can fail earlier, while canonical clients that already send
exact identifiers are unchanged.
