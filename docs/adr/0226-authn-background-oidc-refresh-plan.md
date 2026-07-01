# 0226: Authn Background OIDC Refresh Plan

## Status

Accepted.

## Context

Enterprise request paths must not perform OIDC discovery or JWKS network fetches.
Those calls add external latency, couple authentication to IdP availability, and
can create SSRF exposure when issuer or JWKS configuration is wrong.

`prodex-authn` already authenticates only against decoded claims and a cached
JWKS snapshot. It still needed an explicit planning boundary that tells serving
adapters when a refresh is required and where that refresh may run.

## Decision

Add `plan_oidc_jwks_refresh` to `prodex-authn`.

The planner:

- returns no network source while the JWKS snapshot is fresh;
- allows last-known-good cache use during retry backoff without a network source;
- rejects discovery or JWKS fetch planning in `GatewayRequestPath` mode;
- returns a configured JWKS URL or issuer discovery URL only in
  `ControlPlaneBackground` mode; and
- keeps errors stable and redacted.

The authn crate remains HTTP-client-free and does not fetch discovery documents
or JWKS material itself.

## Consequences

Gateway and control-plane adapters can migrate legacy OIDC refresh behavior to a
bounded background refresh path without changing token validation semantics.
Request-path authentication continues to fail closed when no usable cache exists.
