# Shared ping kernels preserve existing reports (2026-10-23)

Existing status-label and summary algorithms have typed internal Mojo owners.
Their versioned C exports keep the same validation, seven output counters and
error behavior. Labels retain their exact bytes and count plans retain temporary
status precedence. A typed strided count result serves the following native-row
report kernel without calling an imported C export or duplicating its algorithm.
The existing Rust renderer remains unchanged; no new Rust semantic transfer is
claimed for this preparation.

Eleven complete stdout cases were captured before production changes. A test
child process now compares their full frozen bytes, including optional and empty
metadata, all fifteen statuses, Unicode, u128 latency endpoints, Duration::MAX
and u64 JSON endpoints. It mutates no process-global environment and contacts no
provider. The initial frozen harness failed on the child test-name prefix;
`--quiet` corrects only that harness. Its failed log remains in evidence.

The following complete feature reached 1,226 changed lines before documentation,
above the unchanged 1,200-line guard. Shared kernels and frozen caller coverage
are isolated first; the owned feature WIP remains preserved. Direct Mojo and
strict core/rich legacy bridge and ping caller suites validate this extraction.
Direct Mojo, 16 legacy bridge tests in each core/rich mode and 27 ping callers
pass, including all eleven frozen outputs. Strict workspace Clippy, formatting,
ownership/boundary guards and offline compatibility pass.
Canonical share is 36.680630234688245% (138,962 Mojo / 239,881 Rust production LOC).
This preparation transfers no new Rust semantics; the 75% target remains open.
Default changed tests pass with zero Rust tests selected; the explicit core/rich
and caller commands above supply that coverage. Docs, churn and whitespace pass.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/ping_report_shared_kernels_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-app --lib app_commands::ping:: -- --test-threads=1`

Remaining strict Clippy/fmt, guards, offline compatibility, docs, changed tests
and whitespace follow the [matrix](external-model-schema-mojo-20261022.md).
The Rust rich surface requires `mojo-rich`; `mojo-core` includes it, while
`mojo-runtime` alone does not expose these wrappers. Full default CI runs with
the immediately following complete report checkpoint. The 75% goal remains open;
native non-Linux, live providers and performance remain outside coverage.
