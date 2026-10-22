# Runtime launch dry-run reports compiled in Mojo (2026-10-23)

The complete plain launch report and application completion rows now use
compiled defaults, labels, section order, proxy wording, empty-argument handling,
env/removed-env rows, profile labels, Presidio text and exact newline decisions.
Their former Rust schemas are deleted without a fallback. Rust retains OS
display, SocketAddr serialization, config/filesystem acquisition and the existing
argument, env and path-redaction adapters. Redaction rules are not copied.
The existing public CodexConfigResult contract remains unchanged; valid native
formatting failures are internal invariants like the existing redaction bridge.
The application completion port propagates errors through its existing Result.
TUI layout, height and color classification keep their existing owners.

The version-1 native ABI accepts six observed string views followed by ordered
args, env pairs and removed keys. A second operation accepts the base report
and an optional extra row. Presence bits distinguish absent and present-empty
provider/model/extra values. Checked count subtraction avoids multiplication or
aggregate overflow. Version, operation, flags, port range, alignment, UTF-8,
phase and capacity are validated before output. Short capacity leaves output
and metadata unchanged. New typed ports reuse the existing uncapped allocator
and `[present, length]` validation, now named render_native_text. The internal
string-view layout and JSON status codes stay unchanged; rich ping keeps its
different contract. No new dependency or output-size cap is introduced.

Before production edits, seven complete reports were captured as 14,683 UTF-8
bytes. The fixture quotes each newline-inclusive segment to retain whitespace.
Defaults, present-empty values, env order/duplicates, empty entries, IPv4/IPv6,
proxy preview/enabled state, Unicode/escaping, long text and
synthetic-token redaction retain their exact output. All 83 owning-crate tests
pass before and after adoption. The corpus creates no filesystem artifacts.
Two real CLI completion cases also pass on the original implementation.

Adding report exports crossed the launch facade's near-limit threshold and
raised the existing near-limit count to 33, above 32. The complete inspection
model and checked ABI adapter are mechanically moved to a private inspection
module, preserving public re-exports. Both bodies match the published parent.
The size guard passes again. No size or churn limit is changed.
Initial draft logs retain an ambiguous Extend conversion and misplaced inner
module docs during extraction; explicit StringView conversion and restored doc
order correct them.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-runtime-launch --lib capture_original_runtime_launch_reports -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-runtime-launch --lib -- --test-threads=1`
- `cargo test --locked -q --test auto_rotate super_dry_run_ -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_launch_report_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib launch::report::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib launch::report::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --test launch_args`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --test launch_args`

The shared native metadata/session/token/bar feature and application regressions
use the commands in [status-bar-presentation-mojo-20261023.md](status-bar-presentation-mojo-20261023.md).
Strict workspace Clippy/fmt, guards, boundary self-tests, offline replay,
default CI/changed tests, docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core target was invalidated
after harvesting original reports and CLI completion evidence.

Canonical share is 36.80807389668149% (139,867 Mojo / 240,123 Rust production LOC).
Typed host inputs, checked native adapters and mechanical module wiring add
115 net Rust production LOC while transferring the report decisions. Cookie
and info-panel WIP remain protected. The 75% goal stays open. Native non-Linux,
live providers, exhaustive serialized suites and benchmarks are outside coverage.

Direct Mojo, two report bridges, nine shared native regressions and 11 inspection
tests in each core/runtime mode pass. All 83 owning-crate, two real CLI, 20 status
and seven session callers pass, with strict workspace Clippy/fmt and guards.
Full default CI passes 1,451 Rust tests in 226.14 seconds. Default changed tests
pass 83 Rust tests in 77.22 seconds; explicit feature/caller commands cover the
core/app paths whose no-feature changed runs select zero tests.
