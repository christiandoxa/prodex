# Quota provider filter parser and membership in Mojo (2026-10-23)

The quota-watch owner now parses all provider aliases, trims Unicode edge
whitespace, folds ASCII case, replaces underscores, and emits the exact existing
invalid-filter diagnostic. Native profile membership is also Mojo-owned. The
former Rust alias table, normalization chain and membership decisions are
removed. Rust retains typed profile-variant projection without credentials,
checked tag-to-enum mapping, CLI error materialization and existing filesystem
configuration reads. The parse and filter-cycle callers share one tag adapter.

The focused `quota_watch_provider_filter.mojo` module extends the existing
799-line quota-watch policy owner without enlarging it. It reuses that owner's
ASCII fold and versioned filter tags. Both new C ABI exports are reachable from
the production runtime feature. Measured diagnostic output reuses the existing
native text allocator; versions, tags, UTF-8, phases, capacities and alignment
are validated before output. Rejected input and short buffers leave metadata
and output unchanged. There is no parser or membership fallback in Rust.

With `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0`, validation passes:

- `rtk cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib quota_watch_policy::provider_filter` (3 tests; all aliases, normalized errors, Unicode/NUL and long input, the complete native-provider/filter matrix, and invalid ABI records).
- `rtk cargo test --locked -q -p prodex-app --lib quota_provider_filter -- --test-threads=1` (3 caller tests).
- `rtk cargo test --locked -q -p prodex-app --lib quota_support:: -- --test-threads=1` (104 tests, including cycling, rendering, refresh and virtual-provider behavior).
- `rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.

Object generation, both versioned exports and absence of Modular runtime
references pass for all six release triples with their release CPU choices.
Ownership, authority, no-fallback/self-test, crate-boundary/self-test, size,
production-share, formatting, documentation and whitespace checks pass.

The earlier campaign CI now completes successfully, and the Connection-header
checkpoint's 459 serialized proxy tests and changed-path checks pass. The older
runtime-smoke process compiled this uncommitted slice and caught an unqualified
`anyhow!` macro; the adapter now uses `anyhow::anyhow!`. The guard's positive
fixture was extended for the two new authoritative calls. The corrected focused
tests above pass, and runtime-smoke is being repeated without a full CI restart.

Canonical share is 36.912987776704334% (140,606 Mojo / 240,306 Rust production LOC).
The 75% target remains open; no guard or counting rule is weakened. Default quota
refresh, one-shot modes, provider configuration lookup and label text retain
their contracts. Native macOS/Windows execution, live providers and a release
are outside this checkpoint. The pre-existing Docker scaffold omission remains
documented in the stable-toolchain report. Active smoke targets and logs remain
protected until their final consumer finishes; prior cleanup harvested evidence
before removing 16.8 GiB of build output.
