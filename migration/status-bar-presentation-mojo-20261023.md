# Status header and footer compiled in Mojo (2026-10-23)

The live status header and footer now use compiled title, profile prefix,
loading defaults, refresh state, timestamp spacing, keyboard hints, error text
and span-tone decisions. Their former Rust text assembly is deleted without a
fallback. Rust retains terminal geometry, widgets, style application and observed
snapshot/error projection. The private status span adapter shares the existing
token palette and preserves its five original styles.

The version-1 native ABI accepts zero or two header string views, zero or one
footer error view, and explicit presence/refresh bits. Missing snapshots and
present empty fields remain distinct; text is never trimmed. A present empty
error selects the error footer. Version, operation, flags, counts, alignment,
UTF-8, phases and capacities are validated before output. Short capacity leaves
output and metadata unchanged. Tests cover controls, Unicode, quotes, NUL,
long text, invalid views and invalid ABI fields.

The session, token and bar bridges now share their identical native JSON
metadata validation and existing uncapped measured allocation. Their JSON status
codes and `[present, length]` metadata remain unchanged. The rich ping bridge
keeps its different status/error contract. Two new metadata tests cover presence,
negative/changed lengths, ABI errors and invalid UTF-8. Native row acquisition
remains in each input owner.

New header/footer failures propagate through the status renderer and Ratatui's
existing `try_draw` boundary. The terminal does not flush an incomplete frame
when the callback returns an error. The added draft panic sites are removed;
no fallback display is reconstructed in Rust.

Before production edits, 27 complete original dashboard buffers were captured:
nine loading/live/refreshing/error, empty, escaped/control and long-text cases at
120x40, 80x24 and 60x12. Their frozen byte lengths and SHA256 digests pass on the
original and migrated callers, together with the previous 30 token buffers.
Complete original buffers and independently checked digests remain in external
validation evidence. The original private test fixture is reused without a
production test API.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib capture_original_status_bars -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib app_commands::status::render:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::status:: -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/status_bar_presentation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::native_text`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::native_text`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::session_tui`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::session_tui`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::status_tokens`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::status_tokens`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::status_bar`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::status_bar`
- `cargo test --locked -q -p prodex-app --lib app_commands::session:: -- --test-threads=1`

Strict workspace Clippy/fmt, guards, boundary self-tests, offline replay,
default CI/changed tests, docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core target was invalidated
before adopted source-list validation. Initial prototype compilation needed
an explicit Int64 conversion. One bridge expectation omitted an original
trailing space; the expectation was corrected and the production formatter
retained. Failed logs remain in evidence.

Canonical share is 36.78824849471372% (139,671 Mojo / 239,991 Rust production LOC).
Checked host adapters add 75 net Rust production LOC while transferring the
remaining header/footer text decisions and removing repeated bridge metadata
validation. No dependency, counting rule or guard limit changes. Cookie and
info-panel WIP remain protected. The 75% goal stays open. Native non-Linux,
live providers, exhaustive serialized suites and benchmarks are outside coverage.

Direct Mojo, two metadata, three session, two token and two bar bridge tests
in each core/runtime mode pass. All 20 status callers and seven session callers
pass. Strict workspace Clippy/fmt, guards, boundary self-tests and offline replay
pass. Full default CI passes 1,449 Rust tests in 622.53 seconds. Default changed
tests pass in 5.11 seconds with zero Rust tests selected; the explicit native
and caller commands above supply the relevant coverage.
