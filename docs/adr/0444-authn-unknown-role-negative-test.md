# ADR 0444: Authn rejects unknown role claims closed

## Status

Accepted.

## Context

Enterprise authentication must never infer administrator privileges from an
unmapped or missing role claim. The domain mapper already denies unknown claims,
but the authentication boundary also needs direct regression coverage.

## Decision

`prodex-authn` now has a negative test for an unmapped role claim. The boundary
returns `RoleClaimError::Unknown`, and the client response plan stays on the
stable `role_not_authorized` envelope without echoing the claim value.

## Consequences

This is a characterization/security regression test only. It preserves existing
behavior and documents that role mappings must remain explicit.
