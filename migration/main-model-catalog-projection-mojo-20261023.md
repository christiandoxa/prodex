# Main model catalog field projection compiled in Mojo (2026-10-23)

Main-picker model IDs, labels, priority defaults, visibility/capability flags,
effort extraction, aliases and default-effort observations now use a compiled
catalog projection. The former Rust parser, private observation type, three
field helpers and priority constant are removed. The real caller passes the
compiled observations to its existing compiled catalog planner; selection,
sorting, deduplication and effort validation retain that single authority.
There is no Rust fallback.

The focused catalog owner reuses the existing parsed-tree validator, Unicode
trim, exact ASCII comparison, unsigned parser and measured JSON writer. A new
version-1 C boundary validates its reserved arguments, phase, alignment,
counts, UTF-8, tree links, raw spans and root-array shape. Capacity failure
leaves output and metadata unchanged. The source belongs to the production
build graph for the core, rich and runtime feature families. Rust retains the
shared Serde array acquisition, typed result decoding and existing plan adapter.
The provider port reuses the existing catalog error contract; the caller retains
the catalog's existing fatal bridge-failure behavior instead of recomputing.
No dependency or counting-rule change is introduced.

The first typed string wins before Unicode trim: a blank primary blocks its
alias, while a non-string primary permits the alias. Display labels fall back
to the selected ID. Priority preserves every unsigned value, including zero
and u64::MAX, and defaults to the original i64::MAX value for other types.
Only typed false disables API support; only typed true enables hidden status.
Visibility uses exact ASCII-insensitive `list` matching without trimming.
Effort strings remain untrimmed; aliases are trimmed and filtered without
removing duplicates. Input order, empty/scalar entries and optional default
metadata are preserved for the existing planner. This differs from the runtime
catalog's presence-first provider aliases, so that parser cannot be reused.

Twelve complete choice outputs were captured through the original production
caller before adoption. Frozen byte lengths and SHA256 digests cover every
choice, sentinel, label, alias, effort and default. Complete observations and
outputs remain in validation evidence. The same frozen caller passes before
and after migration. Cases include empty/scalar entries, malformed metadata,
blank primary fields, Unicode and JSON escaping, exact flags, duplicate/order
behavior, unsigned endpoints, 4,097-byte fields and excessive raw entries.
A separate provider boundary check compares complete typed observations.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib super_main_prompt::catalog:: -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib super_main_prompt::catalog:: -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/main_catalog_projection_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::main_catalog_projection`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::main_catalog_projection`
- `cargo test --locked -q -p prodex-provider-core --lib catalog::main_projection:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_main_prompt:: -- --test-threads=1`

Strict workspace Clippy/fmt, guards, offline replay, default CI/changed tests,
docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core target was invalidated
before adopted ABI validation. An initial caller filter selected zero tests;
the minimum-test gate rejected it, and the corrected namespace passed seven
original caller tests. The frozen original caller also passed seven tests.
Cookie and info-panel WIP remain protected. No guard cap changes. Native
non-Linux, live providers, exhaustive serialized suites and benchmarks are
outside coverage. The 75% target remains open.

Direct Mojo, two bridge tests in each core/runtime mode, seven catalog callers,
17 main-prompt callers and the provider projection boundary pass. All twelve
original complete-choice digests match. The complete provider suite passes
557 tests. Strict workspace Clippy/fmt, guards, boundary self-tests and offline
replay pass. Canonical share is 36.76933094665025% (139,476 Mojo / 239,851 Rust
production LOC), removing 41 net Rust production LOC.

Full default CI passes 1,447 Rust tests in 589.64 seconds. Default changed tests
pass 557 Rust tests in 141.58 seconds; their no-feature app/core invocations select
zero tests, covered by the explicit caller/feature commands above.
