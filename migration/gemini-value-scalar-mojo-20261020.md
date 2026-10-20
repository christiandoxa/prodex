# Gemini tool-pattern parsing and context-name exclusions (2026-10-20)

## Migrated behavior

The existing production `gemini_value_policy.mojo` owner now handles two
additional deterministic operations through one versioned scalar ABI:
parsing an optional `tool(pattern)` string and classifying exactly the
context-directory names excluded from Gemini workspace inspection.

The tool-pattern parser preserves the original semantics: Unicode trim of
the overall value, first opening parenthesis, last closing parenthesis,
separate Unicode trim of the tool and pattern, and rejection when either
is empty or the close does not follow the open. Bytes after the last close
remain ignored just as in the old parser. Mojo returns validated byte
bounds; Rust only materializes borrowed UTF-8 slices into caller-owned
`String`s. The path-name classifier retains strict case-sensitive matching
of `.git`, `node_modules`, `target`, `dist`, `build`, `__pycache__`, and
`vendor`, without a Rust name table or fallback predicate.

## Verification

The direct compiled scalar ABI tests cover Unicode trim, nested or unmatched
parentheses, empty parts, trailing text, exact exclusions, invalid pointers,
unknown versions, invalid UTF-8, and unchanged output on rejected shapes.
Public provider callers check the same behavior. The full provider-core
suite passes: 512 tests under the strict Mojo build feature.

Executed:
- `mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-gemini-value-scalar-20261020.o mojo/prodex_core/gemini_value_policy.mojo`
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_gemini_scalar -- --test-threads=1` (3 passed)
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-provider-core --features mojo --lib -- --test-threads=1` (512 passed)
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test` (passed)
- `node scripts/ci/mojo-production-share.mjs --check --json` (passed)
- `cargo fmt --all -- --check` and `git diff --check` (passed)

The official broad metric increased from 35.737343386031895% to
35.74326074615431% (129,703 Mojo / 233,171 Rust LOC). Non-regression
holds and the project-wide 75% target remains unmet. No counting rules,
waivers, or baselines were altered.
