# 0421: Domain ID parse display redaction

## Status

Accepted

## Context

Domain identifier parsing rejects malformed tenant, principal, request, call,
reservation, virtual key, policy revision, and audit event IDs. The API error
planner already redacts invalid values, but `Display` text may still be used by
generic logging or serde deserialization errors.

## Decision

`IdParseError` no longer stores or displays the raw invalid identifier value. It
keeps only the identifier kind.

## Consequences

- Malformed identifier values do not leak through `Display`, `Debug`, or generic
  error plumbing.
- Error codes still preserve the identifier kind for clients and operators.
