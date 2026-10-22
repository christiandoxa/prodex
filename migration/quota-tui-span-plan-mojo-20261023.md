# Quota TUI span classification compiled in Mojo (2026-10-23)

Human quota lines and table status cells now use compiled style precedence,
case-sensitive substring/prefix matching, separator classification, Unicode
whitespace and compact-field boundaries. Rust's former classifiers and field
splitting are deleted without a fallback. Ratatui styles, widgets, terminal
geometry and line iteration remain in Rust.

The existing quota presentation owner now returns the compact label's colon
boundary to both its unchanged JSON operation and the new native planner.
The label set is not copied. Existing exact-search, UTF-8 and Unicode-whitespace
workers are reused. Human lines keep their broader status rules; table cells
keep their narrower rules. Headers and compact labels precede status words,
status words precede PROFILE/detail/hint handling, and error words precede
success words. Empty lines and dash/whitespace separators stay muted.
Fields preserve their leading label whitespace and trailing value whitespace.

The new version-1 ABI returns three caller-owned words: a validated tone or
field tag, label end and value start. It validates version, mode, signed byte
length, UTF-8, output alignment and capacity before writes. Short capacity and
invalid input preserve output. Rust validates all tags, unused words, ordering,
slice bounds and UTF-8 boundaries before borrowing the returned field slices.
The fixed-output boundary does not acquire a JSON arena or copy line text.
The previous compact-label JSON ABI remains available and shares the same
compiled label worker. Native input avoids imposing that arena's 64 MiB limit;
a bridge test checks a larger field and verifies that its value is borrowed.

Before production edits, 186 complete original span/cell-style objects and four
complete Text objects were captured and frozen. Their aggregate includes every
input, text, style and line boundary. Cases cover all 23 compact labels, empty
values, Unicode whitespace, non-whitespace controls/zero-width characters,
quotes, NUL, long text, CRLF and competing style rules. The exact 52,962-byte
payload and SHA256 digest match before and after migration. The complete
original payload remains in external validation evidence. All 50 quota-watch
tests pass on both the original and migrated callers.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `cargo test --locked -q -p prodex-app --lib capture_original_quota_tui_spans -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib quota_support::watch -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/quota_tui_spans_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::quota_`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::quota_`

The four bridge tests in each mode cover both new native plans and the
existing JSON label/row contract. Direct tests cover style precedence,
Unicode/control input, empty views, invalid versions/modes/views/alignment,
short output and preserved canaries. The initial Rust build rejected
`Self::Error` because it collided with the tone enum's Error variant. The
return type now explicitly names MojoError; the failed log is retained.

Strict workspace Clippy/fmt, guards, boundary self-tests, offline replay,
default CI/changed tests, docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. The owned core target was invalidated
after harvesting the original baseline. No dependency, counting rule or guard
limit changes.

Canonical share is 36.798752870294294% (139,744 Mojo / 240,008 Rust production LOC).
The native typed adapters add 17 net Rust production LOC while removing the
former Rust classification and splitting decisions. Cookie and info-panel WIP
remain protected. The 75% goal stays open. Native non-Linux, live providers,
exhaustive serialized suites and benchmarks are outside coverage.

Direct Mojo, all four bridge tests in each core/runtime mode, all 50 watch
callers, strict workspace Clippy/fmt, guards, boundary self-tests and offline
replay pass. Full default CI passes 1,450 Rust tests in 609.03 seconds. Default
changed tests pass in 4.91 seconds with zero Rust tests selected; the explicit
feature/caller commands above supply the relevant coverage.
