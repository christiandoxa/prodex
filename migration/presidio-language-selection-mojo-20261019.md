# Presidio language selection — 2026-10-19

The real auto-language inspection caller now delegates substring keyword
scoring, configured-language availability and first-candidate tie priority
to redaction_language.mojo. Rust's keyword tables, lowercase allocation,
score loops and selection branch tree are deleted. The typed versioned
ABI returns a validated configured-candidate index or explicit absence;
ABI errors propagate without a Rust language-selection fallback.

The Mojo matcher preserves Rust Unicode lowercase semantics for ASCII
keywords without allocating a lowercased copy. ASCII uppercase maps to
lowercase; Kelvin sign maps to k; capital I with dot maps to i followed by
a combining dot. An i may end a keyword at that expansion, but cannot
skip its combining dot to match another character. A Rust scalar audit
checks every Unicode scalar to detect future additions to these ASCII
expansions. Keyword occurrences count once, including substring matches.

The unchanged official production-share metric rises from
35.229987052415396% at db65934a4 to 35.240308433950055%:
126,321 Mojo and 232,135 Rust production lines. The remaining gap to
75% is 39.759691566049945 percentage points.

Validation from the repository root:

- PASS: `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_language_test.mojo`.
- PASS (3 tests): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib redaction::language::tests`.
- PASS (5,000 seeded vectors): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib temporary_differential_language_selection -- --test-threads=1`.
- PASS (37 final tests; also 38 including temporary parity coverage): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio:: -- --test-threads=1`.
- PASS: no-fallback self-test/source guard, authority guard, hot-path self-test/source guard, crate-boundary self-test/source guard, official production-share check, formatting, Markdown lint and diff checks.

The temporary Rust comparison oracle was removed after parity verification.
The real linked Rust ABI tests and independent C ABI probe select the correct
candidate. The direct Mojo test covers Unicode substring matching; Rust owns
the caller-lifetime ABI regression. The preceding smoke, manifest and 459-test
proxy checks pass. Existing size and Clippy failures remain documented in the
JSON-selection checkpoint; full CI and native macOS/Windows tests were not run.
