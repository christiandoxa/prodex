# Session TUI row presentation compiled in Mojo (2026-10-23)

Both inline and scrollable session rows now use compiled Mojo title/default
selection, labels, spacing, conditional cwd rows, separator lines and tone order.
The two Rust implementations are deleted without a fallback. A focused owner
reuses the existing UTF-8/Unicode trim and measured JSON sink kernels; the
oversized session metadata owner does not grow. Rust retains observed field
projection, typed span decoding, Ratatui style/object construction and terminal
I/O. Scroll lines are computed once per unchanged report list before redraws.
Headers, footers, viewport sizing, keyboard handling and plain/JSON reports keep
their existing owners.

The version-1 ABI borrows 104-byte native rows aligned to eight bytes, with
presence bits and six string views. Missing and present-empty fields remain
distinct. Counts are bounded by addressable row storage; string lengths retain
the signed-byte bound.
Version, mode, phase, alignment, flags, UTF-8 and capacity are checked before
output. Measured aggregate overflow, invalid fields and short buffers preserve
output and metadata. Typed host decoding validates returned report counts and
tone tags without reconstructing row decisions.

Existing parsed-tree and measured-text runners impose a 64 MiB acquisition or
output limit. Native rows avoid adding that restriction to the original UI.
The existing ping adapter's checked measured-string allocation is extracted into
the private text-output owner and reused by both real callers. Its allocation,
length equality and UTF-8 checks remain unchanged; the older capped text ABI
retains its own contract. This is host allocation, not a Rust display policy.

Before production changes, six complete inline ListItems and the full/empty
scroll span lists were captured from the original functions and frozen in
`session/presentation_baseline.json`. Missing/empty values, ordinary metadata,
Unicode, multiline/quoted values, Unicode blank titles and non-whitespace zero
width characters retain all text, span styles and line boundaries. The original
seven caller tests pass before adoption. Captured empty separator lines contain
zero spans; the draft was corrected to emit that exact original representation.
No process-global environment mutation or provider access is used.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_tui_presentation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::session_tui::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::session_tui::`
- `cargo test --locked -q -p prodex-app --lib app_commands::session:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich --lib rich::ping_protocol::`
- `cargo test --locked -q -p prodex-app --lib app_commands::ping:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib text_output::`

Strict workspace Clippy/fmt, guards, offline compatibility, default CI/changed
tests, docs, churn and whitespace follow the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. Direct Mojo, three native bridge tests in
each core/runtime mode, seven session callers, 19 ping bridge tests in each
core/rich mode and 27 ping callers pass. Both allocation tests pass, including
native output one byte above the legacy 64 MiB cap and changed-length rejection.
Strict workspace Clippy/fmt, guards and offline compatibility pass. Full default
CI passes 1,446 Rust tests in 521.81 seconds. Default changed tests pass with
zero Rust tests selected; explicit feature/caller commands above supply coverage.
The no-fallback self-test initially exposed that the common feature-off pattern
did not include mojo-runtime. That spelling is now rejected for declared
unconditional Mojo owners; no guard limit is relaxed. Initial draft compilation
also required explicit Mojo view/row copies. Corrected direct tests pass.

Canonical share is 36.72706568327156% (139,330 Mojo / 240,036 Rust production LOC).
Native/Ratatui adapters add 85 net Rust production LOC while removing their
former semantic ownership. No dependency or counting rule changes. The owned
core target was invalidated before adopted source-list validation. Cookie and
info-panel WIP remain protected. The 75% goal stays open. Native non-Linux,
live providers, exhaustive serialized suites and performance are outside coverage.
