# 0178: Domain Audit Digest Stable Validation Responses

## Status

Accepted

## Context

Append-only audit chains depend on digest values for tamper evidence. Digest
inputs may come from persisted storage, migration tooling, restore workflows, or
control-plane APIs. Invalid digest values can include copied secrets, provider
payload fragments, oversized high-cardinality strings, or raw route material.

`AuditDigest` already rejected empty values and exposed a stable response
planner. Regulated deployments need bounded validation and a redacted response
plan for non-empty invalid digest material as well.

## Decision

`AuditDigest::new` validates that digest values are non-empty, at most 128
characters, and composed from lowercase ASCII letters, digits, colons, dots,
underscores, and hyphens.
Only zero-length values are empty; whitespace-only values fail as invalid
characters.

`plan_audit_digest_error_response` maps empty digests to
`audit_digest_required` and all other validation failures to
`audit_digest_invalid`. The client-visible response does not echo digest values,
lengths, chain material, tenant identifiers, principal identifiers, resource
identifiers, or credential-like material.

## Consequences

- Audit chain adapters can reject malformed digest material before persistence
  or API exposure.
- Existing valid digest forms such as `sha256:...` remain accepted.
- Raw invalid digest strings remain available only to trusted diagnostics.
