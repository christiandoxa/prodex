# Terminal title sanitization migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled terminal_layout.mojo production module now owns
the terminal title sanitizer through prodex_terminal_title_sanitize_v1.
The kernel removes Unicode General Category Cc controls (U+0000–U+001F
and U+007F–U+009F), preserves other UTF-8 scalars including format
characters, and selects the stable prodex default when no characters
remain. The typed adapter checks ABI version, byte counts, valid UTF-8,
bounds, and output allocation.

Rust only acquires the OS current-directory basename using Path and
to_string_lossy and passes its text to compiled Mojo. The earlier Rust
control-character filter, empty-title decision and default-title
constant are deleted without any fallback. Terminal-specific process
effects remain Rust host duties.

## Evidence

The direct core regressions cover mixed Unicode, C0/C1 controls,
empty/default titles, preserving whitespace, malformed UTF-8,
unsupported versions, null pointers, insufficient capacity and no
output writes on invalid ABI shapes. The real terminal-session caller
regression covers basename acquisition and the compiled sanitizer.

Commands:
- mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-terminal-title-20261020.o mojo/prodex_core/terminal_layout.mojo
- PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib compiled_terminal_title -- --test-threads=1
- PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-terminal-ui -- --test-threads=1
- node scripts/ci/mojo-no-fallback-guard.mjs --self-test
- node scripts/ci/mojo-production-share.mjs --check --json
- cargo fmt --all -- --check
- git diff --check

The focused core ABI tests (2), terminal-crate tests (39), compiler,
no-fallback guard, format and official production-share checks pass.
The canonical metric rises from 35.714305415730024% to
35.72208587265256% (129,539 Mojo and 233,091 Rust production lines).
The 75.0% project objective is still unmet; audited reachable-source
non-regression remains satisfied.
