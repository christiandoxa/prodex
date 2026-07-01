# ADR 0003: Data-plane credentials do not bypass virtual-key accounting

## Status

Accepted.

## Context

Phase 0 security audit requires control-plane or gateway/root credentials not to
be usable as an inference or quota bypass.

The local rewrite gateway supports virtual keys for data-plane budget and usage
accounting. It also supports a gateway/root bearer token for legacy local bridge
access and admin endpoints. When virtual keys were configured, presenting the
gateway/root token to `/v1/responses` skipped virtual-key validation and
accounting.

## Decision

When active virtual keys exist, data-plane requests must present a valid virtual
key. The gateway/root token no longer bypasses virtual-key admission or usage
accounting on inference routes. Admin routes remain authenticated separately
before data-plane policy enforcement.

## Consequences

- Deployments using virtual keys get enforced data-plane accounting for every
  inference request.
- Legacy single-token bridge mode remains available only when no virtual keys are
  configured.
- Admin credentials and data-plane virtual keys remain separated by route and
  policy.
