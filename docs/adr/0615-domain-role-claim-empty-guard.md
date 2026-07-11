# ADR 0615: Domain Role Claim Empty Guard

## Status

Accepted.

## Context

Admin privileges must come from explicit role mappings. Empty or whitespace role
claims should not be usable as explicit mappings because they can appear when an
identity provider omits or serializes a missing claim poorly.

## Decision

Role mapping construction ignores empty configured claim keys. Runtime role
lookup treats absent or zero-length incoming claims as missing.

## Consequences

Blank and whitespace-bearing role claims resolve to denial or Viewer fallback,
never Admin. Existing explicit non-empty role mappings continue to work.
