# ADR 0678: Gateway Admin Role Values Match Exactly

## Status

Accepted.

## Context

Gateway admin roles are authorization inputs. The policy validator and runtime
role parser accepted role strings after trimming whitespace, so padded values
such as `" admin "` could be normalized into Admin.

## Decision

Gateway admin role parsing now requires exact non-empty values without
whitespace. This applies to configured admin token roles, SSO default roles, SSO
role claims, OIDC role claims, and SCIM role values that pass through the shared
role parser.

## Consequences

Canonical role aliases such as `admin`, `viewer`, `write`, and `read-only`
remain supported. Whitespace-bearing role values fail closed or resolve to
Viewer instead of being normalized into write access.
