# Presidio analyzer merge — 2026-10-19

The live multi-language inspection engine now delegates analyzer ordering
and overlap reduction to redaction_analyzer.mojo via the versioned
prodex_redaction_analyzer_merge_v1 ABI. Rust marshals borrowed records and
materializes selected source metadata and ranges. The Rust comparator,
duplicate/containment decisions, overlap range union and stronger-result
replacement helpers are deleted without a fallback.

The compiled owner uses in-place heapsort with the original source index
as a final key to preserve stable ties, followed by the existing adjacent
reduction semantics. Equal-score longer contained findings still leave
the prior finding intact. Expanding a partial overlap may mean a later
same-source range remains separate; this existing behavior is preserved.
Entity and language metadata always come from the selected source record.

Input ranges, finite scores, UTF-8 views, counts, capacity and ABI version
are validated before output publication. Non-finite scores cannot arrive
through Serde JSON and are rejected at the typed boundary. The linked
production object has no undefined runtime symbols or heap dependencies.

The unchanged official metric rises from 35.215954263254275% at 519fd70ec
to 35.229987052415396% (126,253 Mojo / 358,368 production lines).
75% remains outstanding; the gap is 39.770012947584604 percentage points.

Validation from the repository root:

- PASS: `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_analyzer_test.mojo`.
- PASS (1 test): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib redaction::analyzer::tests`.
- PASS (4,000 seeded vectors): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib temporary_differential_analyzer_merge -- --test-threads=1`.
- PASS (36 final tests; also 37 with the temporary differential test): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio:: -- --test-threads=1`.
- PASS: no-fallback self-test/source guard, authority guard, hot-path self-test/source guard, crate-boundary self-test/source guard, runtime manifest guard, offline baseline and five replay fixtures, formatting, docs and diff checks.

The temporary Rust comparison oracle was removed after parity verification.
Permanent caller tests cover the original regression, ordering, overlap and
containment cases, a 4,096-record reverse-order vector, and malformed input.
The preceding checkpoint's runtime smoke and 459-test proxy suite also pass.
Existing size and workspace Clippy failures remain as recorded in the JSON
selection checkpoint. Full CI and native macOS/Windows tests were not run.
