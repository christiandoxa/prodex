# ADR 1002: Gateway root token control-plane denial

## Status

Accepted.

## Context

Phase 0 security requirements separate data-plane and control-plane
credentials. The legacy gateway/root bearer token protects local data-plane
bridge traffic when virtual keys are not configured, but using that same token
as a fallback administrator lets a data-plane credential access control-plane
endpoints.

## Decision

Gateway admin endpoints authenticate only explicit admin tokens, trusted-proxy
SSO, or OIDC identities. Launch configuration no longer promotes `--auth-token`
or `PRODEX_GATEWAY_TOKEN` into a default admin token, and the runtime no longer
uses the legacy gateway/root bearer token as an admin fallback. When no admin
credential source is configured, admin routes return `admin_auth_not_configured`
even if the caller presents the data-plane gateway token.

## Consequences

Legacy data-plane bearer-token mode remains available for inference routes when
no virtual keys are configured. Operators must configure a separate admin token,
SSO, or OIDC for control-plane access. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_auth.rs`.
