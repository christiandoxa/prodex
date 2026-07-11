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

## Phase 2 cutover assessment

The application authentication and authorization boundary now runs on the
production compatibility path, but that policy cutover does not yet provide a
safe single-server transport seam. The executable handler still accepts a
concrete `tiny_http::Request`, and the typed pipeline owns that value through
body capture, `request.respond(...)`, and websocket extraction. Its result is
written to the request-owned socket rather than returned as a typed response.

Hyper cannot adapt `Incoming` to that contract without recreating a transport
bridge. The general `tiny_http` request constructor lives in a private module,
while its public test constructor writes responses to a sink. More importantly,
an in-memory synchronous `Read`/`Write` bridge would need blocking workers and a
separate upgrade channel to preserve backpressure, cancellation, response
ordering, and bidirectional upgrade ownership. That is the same transport
rewrite hidden behind different plumbing, so this phase does not claim the
socket hop has been removed.

The smallest viable migration is:

1. Make the production pipeline consume transport-neutral request metadata, a
   bounded body source, and an explicit upgrade capability instead of
   `tiny_http::Request`.
2. Return a typed response body stream or upgrade handoff. No application stage
   may call `respond`, take a socket, or spawn unbounded per-request work.
3. Keep both thin Hyper and `tiny_http` adapters temporarily and run the current
   response, side-effect, stream-order, cancellation, drain, and upgrade corpus
   against both adapters.
4. Switch the dedicated composition root only after differential parity passes,
   then delete `GatewayBackend` listener ownership and the loopback client.

Until then, `scripts/ci/enterprise-binaries-guard.mjs` pins the compatibility
backend to one ephemeral loopback listener, requires the route-isolated async
front to remain public, and requires backend drain after frontend drain. This
prevents an apparent cutover that merely exposes the blocking listener.

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
cargo test --locked -p prodex-gateway-server -- --test-threads=1
cargo clippy --locked -p prodex-gateway-server --all-targets -- -D warnings
npm run ci:enterprise-binaries-guard
npm run ci:gateway-http-boundary-guard
```
