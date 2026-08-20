# Migration decisions

## 2026-08-20: Rust remains the host

CLI parsing, orchestration, async runtime, HTTP, TLS, persistence, subprocesses,
credentials, terminal lifecycle, and provider adapters remain Rust. This preserves the
existing ecosystem and Prodex runtime invariants.

## 2026-08-20: First seam is quota arithmetic

`remaining_percent` is deterministic, scalar-only, already tested by its callers, and
has no provider, filesystem, time, or security dependency. It proves the build and ABI
without inventing a domain model or exposing a Rust object graph.

## 2026-08-20: Feature is opt-in and component-scoped

`prodex-quota/mojo` is disabled by default. A normal `cargo build` therefore stays
independent of Mojo. When an opt-in build cannot find `mojo` or `ar` on `PATH`, Cargo
emits a warning and uses Rust; explicitly configured tool failures and Mojo compile
errors still fail clearly.

## 2026-08-20: Use object output for this scalar

The local compiler successfully emits an object and Rust links it directly. This avoids
shared-library loader paths and observed runtime dependencies for the scalar function.
Object emission is documented as experimental, so broader migrations must revalidate
this choice or move to a supported artifact/distribution path.

## 2026-08-20: No new Rust dependencies

The integration uses Cargo build-script stdlib APIs and the existing compiler. No FFI,
serialization, networking, or build dependency was added.

## 2026-08-20: Quota policy stays scalar and stateless

Window status, pressure-band aggregation, and pair readiness cross the same opt-in C ABI
as the original remaining-percent calculation. Rust still owns model lookup, labels,
missing-window discovery, and all formatting. Mojo receives only bounded integer values
and explicit presence/status tags.

The runtime-proxy candidate planner was not ported in this wave: its hard-affinity,
health, backoff, inflight, and quota-source ordering are coupled policy invariants, and
per-candidate scalar FFI would be finer-grained than the current bridge justifies.

Route-specific quota pressure is a narrower exception: two already-normalized window
observations and one route tag form a bounded batch decision. The Mojo entry point returns
only a pressure-band tag; it does not select profiles or override hard affinity.
