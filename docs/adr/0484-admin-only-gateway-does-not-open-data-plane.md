# ADR 0484: Admin-only gateway auth does not open the data plane

## Status

Accepted.

## Context

Gateway admin tokens are control-plane credentials. A deployment that configured
only admin tokens and no data-plane credential left `/v1/responses` open.

## Decision

When no virtual keys are configured, data-plane requests require the legacy
gateway bearer token if any gateway credential is configured. Admin tokens do not
authorize model traffic.

## Consequences

Admin-only deployments keep protected control-plane routes without implicitly
opening inference routes. Legacy unauthenticated local rewrite mode remains
available only when no gateway credential is configured.
