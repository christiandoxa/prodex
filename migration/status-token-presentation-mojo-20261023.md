# Status token-panel presentation compiled in Mojo (2026-10-23)

The status token panel's body now uses compiled total saturation, labels,
spacing, event/log/profile counts, token breakdown, style tones, loading text
and history-title selection. Its former Rust text assembly and total arithmetic
are deleted without a fallback. Rust retains Ratatui geometry, widget creation,
style application and the existing sparkline data. The real production renderer
decodes the compiled rows through a private terminal adapter.

Human-count and token-efficiency decisions remain in their existing compiled
owner. Typed workers now serve both the unchanged legacy terminal entrypoints
and this presentation. The old arithmetic bodies are moved once, with no copied
formatter or alternate decision engine. The new version-1 native ABI accepts a
128-byte observed record or an absent snapshot, and uses caller-owned numeric
scratch plus measured output allocation. Its layout, alignment, presence bits,
UTF-8, phases and capacities are checked. Insufficient output capacity preserves
output and metadata. All raw profile/timestamp text retains its presence and
escaping, including empty timestamps. There is no new dependency or legacy
output-size cap.

The first draft used raw addresses of temporary numeric values and local numeric
buffers and failed its direct tests. The adopted design uses typed numeric
workers and explicit caller-owned 128-byte scratch. It passes complete-output,
loading, ABI, presence, scratch and output-capacity checks. Failed draft/debug
experiments remain outside the worktree in validation evidence; none is a Rust
fallback or an alternate production formatter.

Thirty complete original Ratatui buffers were captured before production edits.
Frozen byte lengths and SHA256 digests include every visible cell and style in
the existing Buffer debug representation. Ten cases run at 120x14, 80x10 and
24x4: loading, zero, ordinary counters, either missing timestamp, empty timestamps,
Unicode/quotes/newline/NUL, unsigned endpoints, long text and rounding boundaries.
The frozen assertion passes on the original Rust caller and the compiled
production caller. Complete original buffers remain in validation evidence,
with independently checked digests. All 19 status tests pass. The external
runner initially expected 20, then was corrected to the audited 19 cases; no
repository guard or test was removed.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib capture_original_status_token_buffers -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib status_token_panel_preserves_original_complete_buffers -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/status_token_presentation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::status_tokens`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::status_tokens`
- `cargo test --locked -q -p prodex-app --lib app_commands::status:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib info_render::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib info_render::`

Strict workspace Clippy/fmt, guards, offline replay, default CI/changed tests,
docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core target was invalidated
before adopted ABI validation. Cookie and info-panel WIP remain protected.
Canonical share is 36.783193197597974% (139,597 Mojo / 239,916 Rust production LOC).
The checked native/terminal adapters add 65 net Rust production LOC while
transferring the panel's remaining text decisions. No counting rule or guard cap
changes. The 75% target remains open. Native non-Linux, live providers,
exhaustive serialized suites and benchmarks are outside coverage.

Direct Mojo and two token bridge tests in each core/runtime mode pass. All
30 original complete-buffer digests match through the production renderer.
All 19 status callers and 23 existing info-render regressions in each feature
mode pass. Strict workspace Clippy/fmt, guards, boundary self-tests and offline
replay pass.

Full default CI passes 1,448 Rust tests in 616.79 seconds. Default changed tests
pass in 4.64 seconds with zero Rust tests selected; the explicit native and
caller commands above supply the relevant coverage.
