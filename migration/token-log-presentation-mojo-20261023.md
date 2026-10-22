# Token log presentations compiled in Mojo (2026-10-23)

Token-event JSON envelopes, ordered human metadata, TUI content/tones and search
text now use one compiled log presentation owner. Rust's envelope schema,
metadata assembly, token TUI rows and search formatting are deleted without a
fallback. Rust retains Serde wire compatibility, the existing rate formatter,
Ratatui themes, wrapping, terminal geometry and I/O. The native owner decides
request defaults, optional fields, generation suffixes, average visibility,
labels, spacing and field order. The raw one-shot JSON event keeps its existing
Serde owner; only the stream envelope has a separate presentation schema.

The version-1 ABI reuses the existing parsed-tree validator and application JSON
acquisition boundary, including its established limits. Its input is the typed
event, optional prepared rate and existing unavailable-rate text. It checks
version, operation, flags, alignment, counts, tree topology, UTF-8, known unique
fields, unsigned values and optional types. Serde still encodes floating-point
wire values; prepared rate presence preserves Some(NaN) and infinity behavior.
Output is measured before writing, so short capacity leaves output and metadata
unchanged. The checked adapter uses the existing host allocator. No dependency,
counting rule, guard limit or Rust semantic fallback is added.

Before production adoption, the original metadata statements were mechanically
extracted without changing their contents. The original JSON, full rendered
human blocks, Text/style objects and search views were captured for 135 cases
as 1,758,714 UTF-8 bytes. Their SHA256 is
`0b3cdf3bff7765e197d59df4306f69bc505cbdd9b31a3b13cea8768671d56df7`.
The frozen assertion passes on the original implementation. It covers absent
and zero requests/generations, u64::MAX, signed zero, negative and large rates,
NaN and both infinities, Unicode, control/escaping, long fields, widths, viewport
limits and ASCII/Unicode search. Complete original objects and extraction hashes
remain in validation evidence. Serde canonicalization preserves the original
stream JSON key order.

JSON and human output propagate native errors through their existing Result.
The infallible TUI/search adapters retain the surrounding compiled-renderer
invariant contract for validated event acquisition and native output.
Prototype evidence retains explicit Mojo-copy and local-binding corrections,
native golden-object emission ordering and the corrected nested caller module
path. Failed logs are retained separately from successful checks.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib capture_original_token_log_presentations -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib token_log_presentations_preserve_original_complete_objects -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_token_presentation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::log_token::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::log_`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::log_`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`

Strict workspace Clippy/fmt, guards, boundary self-tests, offline replay,
default CI/changed tests, docs, staged churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core package was invalidated
after capturing and freezing the original caller evidence.

Canonical share is 36.853758266608814% (140,153 Mojo / 240,142 Rust production
LOC), removing 37 net Rust production LOC and adding 222 net Mojo production
LOC. Cookie and info-panel WIP remain protected. The 75% goal stays open.
Native non-Linux execution, live providers, exhaustive serialized suites and
benchmarks remain outside coverage.

Direct Mojo and three fresh core token bridges pass. Each core/runtime feature
mode passes all 20 combined log projection regressions. All 105 serialized log
callers pass, including the adopted complete original corpus. Strict workspace
Clippy/fmt, no-fallback/self-test, authority, ownership/share, size, boundary
guards/self-tests, offline replay, docs and staged churn pass.
Full default CI passes 1,453 Rust tests in 620.16 seconds. Default changed tests
pass in 4.85 seconds and select zero Rust tests; the explicit bridge and caller
commands above provide their coverage. Final documentation and whitespace pass.
