# Runtime catalog plans compiled in Mojo (2026-10-22)

Mojo owns candidate order, exact dynamic/static lookup, context precedence,
saturating compact budgets, metadata defaults, priority and the merged hard limit.
Rust keeps typed Serde/host effects and calls the unchanged compiled schema renderer.
Twelve complete original catalogs match their frozen hashes; no Rust fallback remains.

Validation follows the [matrix](external-model-schema-mojo-20261022.md), with both bridges using
`--lib json::runtime_catalog`, direct `mojo/tests/runtime_catalog_plan_test.mojo`
and `mojo/tests/catalog_shared_kernels_test.mojo`, and caller filter `external_catalog_assembly_`.
Direct Mojo, five bridge tests per mode, two frozen callers, 20 external callers,
five DeepSeek callers and 553 provider tests pass; strict Clippy/fmt and guards pass.
Default CI passes 1,443 tests (709.37s); changed tests pass 553 (147.80s).
Canonical share is 36.676715467586014% (138,938 Mojo / 239,880 Rust LOC); 75% remains open.
Failed prototypes and the stale prep archive are retained; typed helpers and targeted cleanup resolved them.
Native macOS/Windows, live providers, exhaustive serialized suites and benchmarks remain untested.
