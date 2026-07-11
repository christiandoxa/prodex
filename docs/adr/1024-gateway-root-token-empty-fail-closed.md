# ADR 1024: Gateway root token values fail closed

## Status

Accepted.

## Context

The legacy gateway/root bearer token is still supported for data-plane bridge
traffic when virtual keys are not configured. Direct launch configuration
previously trimmed `--auth-token` and `PRODEX_GATEWAY_TOKEN`. Empty results
were treated as if no token had been configured, and whitespace-bearing values
were silently normalized into a different bearer token than operators supplied.

## Decision

Gateway launch now fails closed when `--auth-token` or `PRODEX_GATEWAY_TOKEN`
is present but empty or whitespace-bearing. Unset token inputs continue to mean
"no legacy root token"; virtual keys remain the preferred data-plane credential
boundary.

## Consequences

Operators get a startup error for malformed root-token secret wiring instead of
a gateway whose data-plane auth surface differs from the requested launch
configuration. Deployments that intentionally do not use the legacy root token
should leave both inputs unset.
