# ADR 0983: Stage async serve composition roots before removing legacy tiny_http adapters

## Status

Accepted

## Context

`prodex-gateway` and `prodex-control-plane` now exist as dedicated enterprise
binaries, but long-lived serving is still intentionally gated. The remaining
Prodex-owned HTTP surfaces in `prodex-app` still rely on legacy `tiny_http`
accept loops and process-local mutex/thread coordination:

- gateway/admin compatibility paths in `prodex-app`
- `dashboard` loopback UI serving
- `expose` loopback PTY streaming

That legacy shape is acceptable as a compatibility bridge, but it is the wrong
place to finish multi-tenant, multi-replica, and telemetry-complete enterprise
serving. Replacing it in one step would risk proxy invariants, admin route
compatibility, loopback exposure behavior, and the current one-shot enterprise
binary flows.

The dedicated enterprise binaries currently fail `serve` fast with an explicit
exit-2 compatibility message, and `enterprise_binaries` pins that gate so the
staging boundary stays deliberate until real async adapters exist.

## Decision

Keep the shipped enterprise binaries as thin composition roots and migrate serve
ownership in stages:

1. Keep one-shot operational commands (`migrate`, control-plane planning,
   config-publication publish/deliver/consume/compact) on the dedicated
   binaries.
2. Add future async `serve` adapters only after they can call the existing
   boundary crates directly (`prodex-application`, `prodex-gateway-http`,
   `prodex-gateway-core`, `prodex-control-plane`, `prodex-config`, and
   `prodex-observability`) instead of copying `prodex-app` branch logic.
3. Treat loopback-only Prodex-owned surfaces (`dashboard`, `expose`) as
   separate migration targets from tenant-facing gateway/control-plane traffic.
   They may stay local-only during the first async serve cutover.
4. Reuse the shared OTLP/HTTP export path already wired into the dedicated
   binaries so future long-lived serve roots emit the same event schema instead
   of inventing a second telemetry path.
5. Remove legacy `tiny_http` and mutex-backed compatibility code only after the
   async serve roots preserve existing request/response envelopes, admin route
   planning, and loopback security expectations.

## Consequences

- The remaining `tiny_http` and mutex-backed paths are now an explicit staged
  migration target rather than an open-ended cleanup note.
- Enterprise work can keep shipping narrow binary and boundary improvements
  without pretending long-lived serve is complete.
- The shipped `serve` command surface stays explicitly unavailable until the
  adapter cutover lands, which avoids implying enterprise readiness before the
  long-lived path actually preserves the required invariants.
- Future serve work has a clear cut line: gateway/control-plane async serving
  first, loopback dashboard/expose migration separately, then compatibility code
  removal.
