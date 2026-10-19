# Presidio finding localization — 2026-10-19

The compiled redaction finding owner now computes inspected-field ranges,
filters findings crossing JSON field boundaries, localizes Unicode scalar
offsets to UTF-8 byte positions, classifies analyzer entities and rounds
confidence to basis points. Rust retains borrowed input acquisition,
validated result materialization, typed domain construction and transport.

The previous Rust Unicode offset and confidence helpers, entity classifier,
and independent remote filtering range walk are deleted. Both filtering and
strict localization use the same versioned production ABI. Semantic and
ABI failures return explicit errors; they never recompute in Rust.

The ABI validates view lengths, UTF-8, counts, ranges and output capacity.
The host validates every returned index, ordered source position, byte
boundary, kind and confidence. Partial output is not published on failure.
The kernel preserves existing diagnostic strings and known/future entity
mapping, including exact-case _ID/_SSN suffix handling. Unicode separators
count as scalar positions, matching analyzer offsets.

Confidence rounding compares the fractional part against 0.5 directly.
Adding 0.5 before truncation can incorrectly round an adjacent representable
value upward. Thirty thousand linked caller-boundary comparisons against
Rust round cover every half-step and its immediate floating-point neighbors.

The unchanged official metric rises from 35.240308433950055% at ba66567ee
to 35.252231806801646%: 126,442 Mojo and 232,236 Rust production lines.
The remaining gap to 75% is 39.747768193198354 percentage points.

Validation from the repository root:

- PASS: `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_findings_test.mojo`.
- PASS (5 tests): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib redaction::findings::tests`.
- PASS (38 tests): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio:: -- --test-threads=1`.
- PASS: no-fallback self-test/source guard, authority guard, hot-path self-test/source guard, crate-boundary self-test/source guard, offline baseline and five replay fixtures, official production-share and diff checks.

Broad proxy, smoke, manifest and workspace Clippy checks are being finalized.
Existing size and Clippy failures are recorded in the preceding checkpoints.
Full CI and native macOS/Windows execution were not run.
