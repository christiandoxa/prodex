# ADR 1075: Dedicated In-Process Gateway Transport

## Status

Accepted. Supersedes the loopback transport portions of ADRs 1063, 1072, and 1074. The legacy
`prodex gateway` CLI compatibility path is unchanged.

## Context

The dedicated data-plane and control-plane binaries previously composed two HTTP servers: the
public bounded Hyper front and a private TinyServer backend on an ephemeral loopback port. That
extra transport forced the already canonical request target through a second URI parse, added
socket buffering between two admission systems, and split upgrade and drain ownership.

The application pipeline remains blocking in several provider and persistence adapters. Removing
the socket cannot move that work onto async connection tasks or weaken streaming backpressure,
pre-commit error behavior, cancellation, Gemini Live upgrades, or bounded shutdown.

## Decision

Dedicated binaries start one listener-free `RuntimeGatewayApplication` and pass it to
`prodex_gateway_server::serve_with_handler`. The handler moves the front's exact
`GatewayHandlerRequest.target` into the application pipeline; it never serializes or reparses that
target.

Before any request-body pump or blocking pipeline task is spawned, the application acquires a
dedicated bounded request permit. Model traffic fails fast with a stable local 503 when saturated;
health probes use a separate small bounded lane. The permit remains owned by the request pump,
blocking pipeline, response body, or upgrade handoff until the corresponding operation completes.

Request and response channels use explicit data, end, and error messages. A producer disappearing
without an end marker becomes a transport error rather than a clean truncated response. Response
headers are committed before body production, bounded channel sends provide backpressure, and
stream errors remain observable after commitment. Gemini Live receives a bounded in-process
duplex handoff with one WebSocket handshake and no `Content-Length` on the 101 response.

The application owns its async runtime and startup workers. Shutdown closes admission, waits within
the configured drain deadline for request/response permits, accounting tasks, and finished startup
workers, then detaches only workers that exceeded that bounded contract. The Docker default and
dedicated Kubernetes commands invoke `prodex-gateway serve` or `prodex-control-plane serve`; the
root `prodex gateway` command keeps TinyServer compatibility without becoming a production hop.

## Consequences

- Dedicated production requests have one HTTP listener and one canonical request-target object.
- Route-plane rejection remains in the async front before application work.
- Control-plane key and SCIM writes retain that exact target and authorized action through the
  canonical atomic mutation execution defined by ADR 1076.
- Blocking provider and persistence adapters remain isolated behind explicit bounded permits.
- Streaming, cancellation, overload, upgrade, accounting, and drain behavior have focused parity
  tests and CI source guards.
- `GatewayBackend` and the loopback `serve` adapter remain available only for compatibility callers
  and can be removed separately when the legacy CLI path is retired.

## Verification

```bash
cargo test -p prodex-gateway-server --lib
cargo test -p prodex-app --lib direct_ -- --test-threads=1
npm run ci:enterprise-binaries-guard
npm run ci:production-boundary-guard
npm run ci:size-guard
cargo clippy -p prodex-app -p prodex-gateway-server --lib -- -D warnings
```
