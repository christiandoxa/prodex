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
errors still fail clearly. Runtime diagnostics intentionally report
`compiler_required=false`; `build_strict` reflects `PRODEX_MOJO_REQUIRED` and may be true
only for the build gate. A shipped binary can therefore report `compiler_required=false`
and `build_strict=true`.

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
per-candidate scalar FFI would be finer-grained than the current bridge justifies. This
does not block the separate governed provider-routing batch recorded below.

Route-specific quota pressure is a narrower exception: two already-normalized window
observations and one route tag form a bounded batch decision. The Mojo entry point returns
only a pressure-band tag; it does not select profiles or override hard affinity.

## 2026-08-20: Consolidate Mojo build ownership

The repeated per-crate build scripts were replaced with `prodex-mojo-core`. It owns the unsafe
FFI declarations, source selection, archive creation, strict-mode behavior, target flags, and
activation cfgs. Consumer crates expose safe typed wrappers. This keeps the ABI small and makes
the release feature set (`mojo-core`) explicit without making Mojo a default dependency.

## 2026-08-20: Governed provider routing crosses one complete batch

The governed provider plan crosses one flat-buffer batch of at most 64 records. Rust validates
descriptors, evaluates static and policy hard filters, normalizes bounded signals, maps provider
identity to a stable order value, and reconstructs routes and candidate evaluations. Mojo applies
the required capability mask, performs score arithmetic, and orders eligible indices by affinity,
score, provider order, and original index. Rust retains credentials, route construction, affinity
error handling, and user-facing errors.

Provider route capability negotiation uses a separate flat-buffer batch. Rust maps the seven
`ModelCapability` values to a mask and keeps model/route objects and redacted error plans; Mojo
returns compatible flags, reason tags, and first compatible/incompatible indices. Malformed route
tokens remain excluded from first-index selection.

Runtime profile quota scoring and Smart Context byte estimation remain bounded numeric kernels.
Tokenizer integration and candidate selection remain Rust.

## 2026-08-20: Keep accounting and distributed rate limiting in Rust

The generic domain `commit_reservation` and `evaluate_rate_limit` helpers were audited but not
promoted: the durable accounting flow uses `reconcile_reserved_usage`, while distributed rate
limiting is owned by Redis/runtime adapters. Adding an unused scalar wrapper would violate the
zero-unused-Mojo rule. Accounting-budget enforcement, SLO classification, and float-heavy
context ranking remain Rust until a real production seam and exact parity contract exist.

## 2026-08-20: Linux Mojo release is fail-closed

The release matrix enables compiled-in Mojo only for `x86_64-unknown-linux-gnu`. Mojo is compiled
into a target archive in an isolated Cargo target directory, then the final Rust binary is linked
through the existing cross container with the archive path and strict Mojo variables explicitly
forwarded. This prevents host build-script binaries or host GLIBC from defining the deployment
baseline. Other platforms remain Rust-only until final-link, runtime, signing, and clean-machine
evidence exists. The compiled-in binary must still report `compiler_required=false` at runtime;
the release build must report `build_strict=true` under `PRODEX_MOJO_REQUIRED=1`.

## 2026-08-20: Release metadata drives installation

Release CI renders `release-manifest.tsv` and JSON from one target matrix and covers the metadata
with `SHA256SUMS`. `install.sh` and `install.ps1` select by target and verify the staged binary's
own `doctor --runtime --json` implementation and Mojo self-test. They never inspect or install a
user Mojo compiler. WSL naturally uses the Linux shell installer; native Windows remains a
Rust-compatible artifact while its target is not release-approved for Mojo.
