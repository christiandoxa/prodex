# Runtime launch dry-run TUI plans compiled in Mojo (2026-10-23)

The existing launch-report owner now selects title/field/plain line kinds,
the first colon, Unicode value trimming, ASCII case-insensitive color precedence
and inline height clamping. Rust's former classifiers and height arithmetic are
deleted without a fallback. Rust applies Ratatui themes, borrowed field slices
and existing terminal geometry. The header keeps its existing owner. Checked
native failures propagate through the existing Result before terminal drawing.

The version-1 ABI returns four words: line kind, label end, value start and color.
Height mode returns the bounded height with three zero words. It validates
version, operation, signed length, UTF-8, output alignment and capacity before
writing. Invalid or short calls preserve output. The adapter checks enum tags,
UTF-8 slice boundaries, ordering and the height range. Existing Unicode trimming
and ASCII search workers are reused; no dependency or arbitrary text cap is added.

Before production edits, 92 complete input/Text/style/height objects were
captured as 59,094 UTF-8 bytes. Their SHA256 is
`d5f151daf6a52380f3118c2097409a9c92d4fce9814ff39dea9922c3cb3df8a7`.
The frozen caller passes on both the original and adopted implementations.
It covers title precedence, conflicting colors, Unicode and control whitespace,
embedded colons, empty text, CRLF, trailing newlines, all height boundaries and
long values. The original complete objects remain in the validation evidence.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib capture_original_dry_run_tui -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib dry_run_tui_preserves_original_complete_text_and_heights -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_launch_tui_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_launch_report_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib launch::report_tui::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib launch::report`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib launch::report`
- `cargo test --locked -q -p prodex-runtime-launch --lib -- --test-threads=1`
- `cargo test --locked -q --test auto_rotate super_dry_run_ -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::child_process::tests:: -- --test-threads=1`

Strict workspace Clippy/fmt, guards, boundary self-tests, offline replay,
default CI/changed tests, docs, staged churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core package was invalidated
after harvesting the original caller evidence.

Initial guard and self-test runs rejected rustfmt's wrapped include_str!
invocation. The corrected guard requires both the invocation and exact quoted
fixture filename; both checks pass without removing a requirement. Failed logs
are retained separately.

Canonical share is 36.81329088947936% (139,931 Mojo / 240,179 Rust production LOC).
The typed native adapter adds 56 net Rust production LOC while transferring
the presentation decisions; Mojo adds 64 net production LOC. Counting rules and
guard limits are unchanged. Cookie and info-panel WIP remain protected. The
75% goal stays open. Native non-Linux execution, live providers, exhaustive
serialized suites and benchmarks remain outside coverage.

Direct Mojo, four combined report/TUI bridges in each core/runtime mode, all
83 owning-crate tests, two CLI completion callers and six child-process callers
pass. Strict workspace Clippy/fmt and all listed guards pass. Full default CI
passes 1,452 Rust tests in 540.78 seconds. Default changed tests pass in 5.87
seconds and select zero Rust tests; the explicit feature/caller commands above
supply their coverage. Documentation, staged churn and whitespace also pass.
