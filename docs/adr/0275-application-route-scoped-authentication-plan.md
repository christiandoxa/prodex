# ADR 0275: Application Route-Scoped Authentication Plan

## Status

Accepted

## Context

`prodex-authn` validates OIDC/JWT metadata without network access and rejects
missing role claims, stale unusable JWKS, unknown key IDs, and missing tenants.
Application use cases still needed a shared composition point that binds the
authenticated principal to the HTTP route surface before data-plane or
control-plane logic runs.

Without this boundary, adapters can authenticate a token and then accidentally
route a control-plane credential to data-plane quota or inference, or route a
data-plane credential to admin endpoints before downstream authorization has a
chance to reject it.

## Decision

Add `plan_application_request_authentication` to `prodex-application`.

The planner composes `plan_gateway_http_request` with
`authenticate_oidc_claims`, derives the required credential scope from the
classified route, and fails closed when the credential scope does not match the
route:

- inference, compact, websocket, and quota routes require `DataPlane`;
- control-plane routes require `ControlPlane`;
- health and unknown routes are not accepted by this authenticated use case.

Failures are converted to stable redacted response plans. Scope mismatch uses
`credential_scope_not_allowed` without exposing tenant IDs, route internals,
role claims, or credential scope names in the public message.

## Consequences

Composition roots get one reusable request authentication boundary before
calling data-plane admission, quota read, or control-plane use cases. The
application crate now depends on `prodex-authn`, but still has no HTTP
framework, network client, async runtime, storage driver, filesystem, CLI, or
provider SDK dependency.
