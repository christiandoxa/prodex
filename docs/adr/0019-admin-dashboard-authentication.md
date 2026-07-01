# ADR 0019: Require Admin Authentication for Gateway Dashboard

## Status

Accepted

## Context

The gateway admin dashboard is a Prodex-owned control-plane UI. Even though API calls from the page still require admin credentials, serving the UI without authentication exposes internal control-plane routes, feature names, and operational assumptions to unauthenticated users. The enterprise target requires secure-by-default authentication and authorization for control-plane surfaces.

## Decision

The `/v1/prodex/gateway/admin` dashboard route is now classified as an admin route and is served only after the existing gateway admin authentication succeeds. Unauthenticated requests receive the same admin-authentication error path as other admin endpoints. A successful authenticated `GET` still returns the static dashboard with the existing cache and browser security headers.

## Consequences

The dashboard is no longer anonymously discoverable when admin authentication is configured. This is a behavior-hardening change for the control plane only; OpenAI-compatible data-plane pass-through and runtime streaming behavior are unchanged.
