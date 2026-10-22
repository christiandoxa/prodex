# Complete ping reports compiled in Mojo (2026-10-23)

Human profile rows, human aggregate reports and the complete aggregate JSON
schema now belong to the compiled ping-report owner. Optional-field defaults,
first-row metadata selection, credential-validation labels, overall status,
canonical JSON key order and numeric/text formatting are removed from Rust.
The existing typed Mojo status/count kernels supply labels and strided totals;
there is no Rust recomputation or fallback. Rust retains observed field
projection, native ABI storage, allocation, Duration acquisition and stdout I/O.
The production build source list and existing rich exports reach the new owner.

The version-1 ABI borrows 128-byte native rows aligned to eight bytes, with presence bits,
five string views and low/high u64 words for each u128 latency. This avoids
manufacturing JSON or imposing the parsed-JSON acquisition limit on existing
ping fields. The existing measured-text runner also caps output at 64 MiB;
native reports keep their original addressable range. This ABI preserves the
existing 65,536-row summary cap and validates
version, kind, phase, alignment, status, flags, UTF-8, lengths and capacity.
The shared measured sink checks aggregate overflow before writing; rejected
inputs or insufficient output capacity preserve output and metadata.
Human latencies retain the full u128 range, including Duration::MAX. JSON keeps
the original Serde u64 number range; larger values now propagate a typed error
through the existing Result instead of panicking in the former json! macro.
The u128 decimal writer delegates its u64 prefix to the existing sink writer.

Eleven complete stdout cases captured before production changes remain frozen
and unchanged. They cover missing/empty metadata, all fifteen statuses, Unicode,
escaping, multiline details, empty/healthy/mixed/failed aggregates, u128::MAX
human latencies, Duration::MAX and u64::MAX JSON. The production caller test
captures a deterministic child test process without environment mutation or
provider access. The original summary caller also checks its complete summary
object through the real renderer.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_report_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_report_wide_number_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_report_shared_kernels_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-app --lib app_commands::ping:: -- --test-threads=1`

Strict workspace Clippy/fmt, no-fallback/self-test, authority, ownership, share,
size, secret, wildcard, crate/application boundaries and offline compatibility
follow the [validation matrix](external-model-schema-mojo-20261022.md).
Full default `npm run ci -- --jobs 1` supplies the preceding shared-kernel
checkpoint's deferred CI; default CI/changed tests omit strict/archive overrides.
All three direct tests, 19 bridge tests in each core/rich mode and 27 application
ping callers pass, including all eleven frozen outputs. Strict workspace Clippy,
formatting, guards and offline compatibility pass. Full default CI passes
1,445 Rust tests in 542.41 seconds. Default changed tests pass with zero Rust
tests selected; the explicit feature/caller commands above supply that coverage.
Initial checks caught stale ownership consumer proofs, then a duplicate export
claim in their update; the historical shared kernels keep distinct exports and
their consumer proofs follow the composition. Clippy also caught a fixture
mutation hidden behind an earlier integer address; the test now borrows the
mutated row at its final call. Failure evidence remains retained; no guard or
lint was relaxed.

Canonical share is 36.71811128282759% (139,227 Mojo / 239,951 Rust production LOC).
The native checked ABI adds 70 net Rust production LOC while removing its
former report semantics. Existing operation consumer proofs now follow the
composed report and remaining typed status projection. Labels/counts retain
their original shared kernels and C exports.
No dependency, counting rule or guard limit changes.
The owned core target was invalidated before adoption to avoid a stale prep
archive; the clean integrated prep worktree/branch was removed after publication.
Protected cookie and info-panel WIP remain unchanged. The 75% goal stays open.
Native non-Linux execution, live providers, exhaustive serialized suites and
performance benchmarks remain outside coverage.
