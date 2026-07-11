# ADR 1063: Async Gateway Compatibility Front

## Status

Accepted

## Context

The compatibility gateway owns mature provider routing, affinity, streaming,
authentication, accounting, and admin behavior, but its public listener uses
blocking `tiny_http` workers. Rewriting those contracts together would create a
large behavioral and security regression surface. The dedicated enterprise
binaries also need data-plane and control-plane network isolation before their
serve commands can replace the legacy entrypoint.

## Decision

Add `prodex-gateway-server`, a focused Hyper HTTP/1 compatibility front. The
composition root starts the existing backend on loopback and passes only its
socket address to this crate. The crate does not depend on `prodex-app`.

The front provides:

- async accept, request, response, and upgrade I/O;
- a bounded connection semaphore;
- streaming request and response bodies without buffering;
- declared and streaming request body limits;
- pooled loopback HTTP connections;
- a bounded response-header timeout;
- route isolation between data-plane and control-plane listeners;
- byte-transparent WebSocket upgrade tunnels;
- SIGINT/SIGTERM handling, readiness removal through listener closure, and
  bounded connection draining; and
- stable redacted JSON errors.

The loopback compatibility backend remains a bounded blocking dependency during
the migration. It is never called directly on a Hyper executor thread. This is
an explicit transition boundary, not a claim that all upstream and database I/O
has already moved to async adapters.

## Consequences

Streaming, cancellation, and upgrade behavior can migrate incrementally while
the existing gateway contracts remain characterized. The data-plane front
rejects classified control-plane routes; the control-plane front permits only
control-plane and health routes.

The dedicated binaries must own backend startup and final backend drain. The
production deployment remains on the legacy command until binary smoke tests,
credential separation, and multi-replica accounting gates are complete.

Rollback removes the async front from the composition root and returns traffic
to the legacy listener; no persisted state or API contract changes are needed.

## Verification

```bash
cargo test -p prodex-gateway-server
cargo clippy -p prodex-gateway-server --all-targets -- -D warnings
npm run ci:gateway-http-boundary-guard
```
