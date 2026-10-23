# Cookie relay ordering and stable toolchains (2026-10-23)

Mojo owns relay precedence for already filtered cookies: descending path byte
length, ascending cookie-name bytes, and stable source position. The original
Rust comparator is removed. The existing Mojo index heapsort and byte-view
comparison are reused. Rust retains profile/host/namespace isolation, caller
cookie precedence, secure/path eligibility, jar locks, clocks and cookie values.
HTTP and WebSocket callers use the same production batch.

The version-1 ABI accepts at most 4,096 borrowed path-length/name records and
writes an index permutation into caller-owned memory. The live host jar already
caps matching candidates at 32. ABI version, count, alignment, name UTF-8 and
output capacity are checked before writing; Rust validates every returned index
and rejects duplicates. There is no Rust production comparator or fallback.

Stable build pins are Mojo **1.2.0** and Rust/MSRV **1.99.1**, including workflows,
cache keys, release target metadata, Docker's verified official-image digest,
installer fixtures and maintained testing/security documents. Historical reports
retain their original versions. Cargo.lock and package versions are unchanged.
The local stable Rust installation and repository override both report 1.99.1.

Mojo 1.2.0 exposed a recursive `CStringSpan.__init__` resolution error when the
context grouping kernel constructed a string view from borrowed bytes. Hashing
now reads the same validated bytes directly; equality reuses `rich_view_compare`.
The hash algorithm, grouping order, Unicode/NUL handling and ABI remain unchanged.
A caller-boundary regression distinguishes equal-length strings after embedded
NUL. No compiler assertions, ownership guards or runtime checks are disabled.

Validation completed before this checkpoint:

- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo +1.99.1 test --locked -q -p prodex-mojo-core --features mojo-rich --lib` (961 tests).
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo test --locked -q -p prodex-runtime-cookies` (15 tests).
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- PASS: `node --test scripts/ci/ci-pipeline-performance.test.mjs scripts/install.test.mjs` (29 passed; one native Windows installer test skipped on Linux).
- PASS: `rtk cargo fmt --check`, `npm run docs`, `npm run npm:sync-version`, `npm run release:prepare -- --dry-run`, and `rtk git diff --check`.
- PASS: crate-boundary and runtime-hotpath guards and their self-tests; offline upstream baseline and five capture/replay checks.
- PASS: ownership, authority, no-fallback, production-share, size and worktree churn guards.
- PASS: direct cookie object generation for all six release triples with the pinned compiler and release CPU choices. This is object evidence, not native final-link/runtime qualification.
- FAIL: `rtk docker build --target builder -t prodex-toolchain-check:local .`; the existing Docker scaffold omits the required `vendor/tiny_http` patch directory. The new image digest resolves successfully. Deployment-scaffold repair is separate work.
- FAIL: default `npm run ci` reached `generated-metadata-clean`, which rejects the uncommitted Cargo.toml toolchain change. Its metadata preflight, runtime-manifest gate and locked workspace compilation passed. Repeat after the checkpoint is committed. An earlier run rejected incomplete ownership metadata; the corrected ownership and authority guards pass independently.

The early runtime-manifest build was stopped to avoid compiling the old Rust
toolchain; the default CI run passed that gate with Rust 1.99.1. Remote CI is not
a checkpoint prerequisite. Native non-Linux links/execution, live providers,
exhaustive serialized tests and release publication are outside this evidence.

Canonical production share is 36.85708952967805% (140,204 Mojo / 240,195 Rust
production LOC), below the 75% campaign target. The quantity
floor passing does not establish campaign completion. Temporary probe objects and
logs are task-owned build output; cargo cleanup follows their last consumer.
Scoped `trash-empty -f --trash-dir` checked the empty user Trash and removed no
files. No release or tag is created by this checkpoint.
