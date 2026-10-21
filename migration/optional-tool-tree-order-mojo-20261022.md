# Optional-tool digest ordering reuses compiled Mojo (2026-10-22)

## Production ownership

The Caveman and Ponytail installation validators now hash their collected
trees in the existing Mojo lexical file-key order. The Rust string sort is
deleted without a fallback. The existing checked adapter validates a full,
stable permutation; Rust keeps original relative names, paths and lengths and
streams their bytes to SHA-256 in the selected order.

Normalized relative names are valid UTF-8 strings, whose Rust string ordering
matches the existing raw-byte lexical kernel. Deduplication is disabled so
every collected record contributes to the digest. The collection cap is 4,096
files, inside the existing ABI's 1,048,576-record capacity. ABI failures return
an error before hashing any file. No new Mojo source, C export, dependency,
cap or policy configuration is added.

Filesystem acquisition, symlink checks, path normalization, size validation,
bounded reads and SHA-256 remain with their host/dependency owners. Both v1
digest domains, length encoding, manifest exclusion and root `.git` exclusion
retain their established behavior.

## Verification

The existing direct Mojo and strict bridge regressions exercise Unicode byte
order, stable duplicates, empty keys, full permutations and atomic rejection
of malformed trailing views. A new real-filesystem caller fixture checks
fixed complete digests for both Caveman and Ponytail domains, including nested
paths, deliberately unordered creation, Unicode filenames, excluded Git
metadata and the excluded manifest. The full optional-tools crate also
exercises validated installs, content tampering and symlink rejection.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed-test selection omit them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/core_file_lexical_order_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib core_file_policy::lexical_file_key_order_tests::`
- `cargo test --locked -q -p prodex-optional-tools --lib compiled_tree_order_preserves_unicode_nested_digest_bytes`
- `cargo test --locked -q -p prodex-optional-tools`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

Native non-Linux, live-provider and full-workspace tests remain outside this
offline unit. The 75% project target remains open.

The fixture uses `ø`, which has no canonical Unicode decomposition, so
filesystem filename normalization does not change that expected digest key.
The initial equivalent fixture passed on Linux; the portable fixture is
validated again after this test-only refinement.

Direct Mojo assertions, both strict lexical-order bridge tests, the real-tree
digest regression, all 69 optional-tools tests and strict workspace Clippy
pass. Authority, ownership, no-fallback self-test, size, crate-boundary
self-test/check and secret-boundary guards pass. Canonical share is
36.31160555424055% (136,532 Mojo / 239,469 Rust production LOC). Rust's additional
key acquisition slightly lowers raw share; counting rules and guard thresholds
are unchanged.

The portable digest fixture and all 69 optional-tools tests pass again with
Mojo required. The full local CI gate and changed-test runner pass; changed
selection also runs all 69 optional-tools tests. CI passes 685 runtime-launch,
43 auto-rotate and 282 Node tests; its Windows installer test is skipped on
Linux. Final documentation and whitespace checks pass.
