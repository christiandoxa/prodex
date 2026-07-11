# ADR 0680: Gateway Governance Scope Identifiers Match Exactly

## Status

Accepted.

## Context

Gateway admin tokens and virtual keys can carry governance scope identifiers
such as tenant, team, project, user, and budget IDs. These identifiers are used
for tenant isolation, authorization, and accounting scope decisions. Policy
validation only rejected trim-empty values, and runtime config resolution
trimmed configured values before putting them into active state.

## Decision

Configured governance scope identifiers must now be exact non-empty values
without whitespace. Runtime config resolution no longer trims these scope IDs;
if policy settings are built directly, whitespace-bearing values are ignored
rather than normalized into active authorization/accounting scope.

## Consequences

Canonical scope identifiers are unchanged. Padded tenant, team, project, user,
or budget IDs fail closed in policy files and no longer become active scoped
credentials through trim-normalization.
