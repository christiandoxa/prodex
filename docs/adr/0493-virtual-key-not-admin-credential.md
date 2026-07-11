# ADR 0493: Keep Virtual Keys Out Of Admin Authentication

## Status

Accepted.

## Context

Enterprise deployments separate data-plane credentials from control-plane credentials. A virtual gateway key authorizes model traffic and quota accounting, but must not authenticate `/v1/prodex/gateway/*` admin endpoints.

## Decision

Admin authentication continues to accept only configured admin tokens, trusted SSO, native OIDC, or the legacy gateway admin token. Virtual keys are deliberately excluded from the admin auth path and are covered by a regression test.

## Consequences

Client keys cannot read or mutate gateway keys, usage, ledger, SCIM, metrics, OpenAPI, or dashboard endpoints. Operators must issue separate admin credentials for control-plane access.
