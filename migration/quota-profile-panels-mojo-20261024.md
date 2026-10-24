# Quota profile panel schemas in Mojo

The four provider panel callers now use the existing quota presentation owner
through `prodex_quota_profile_panel_v1`. Mojo owns the complete title and ordered
field schema, optional defaults, access-row equality, additional-name precedence
and Gemini bucket numbering. Rust's former assembly and label helpers are
deleted without a fallback.

Rust retains provider observation acquisition, the existing provider formatters,
checked allocation, JSON materialization and the terminal renderer. No provider
or terminal contract changes. The native bridge uses borrowed UTF-8 views and
fixed-width extra rows; it validates presence masks, tags, UTF-8 and capacity
before touching output. It introduces no fixed bucket or text-output limit.

Missing and present-empty values remain distinct. OpenAI's empty additional name
keeps its leading separator, and Copilot still shows Access when its plan is
absent even if the displayed fallback plan equals Access. Gemini Project and
bucket order and External's verbatim details retain their original behavior.

Four field/width parity tests passed against the original Rust renderer before
promotion, then passed after promotion. The complete quota suite passes all 115
tests. Native bridge tests cover defaults, exact presence and equality, invalid
input, capacity canaries, and 2,049 ordered Gemini buckets. The direct Mojo ABI
test also checks exact allocation and untouched output on failure.

The 75% campaign remains open. Native macOS/Windows execution and remote CI are
not claimed by this local checkpoint.

Validation:

- PASS before promotion: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo test --locked -q -p prodex-quota profile_panels -- --test-threads=1` (four original-renderer parity tests).
- PASS after promotion: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo test --locked -q -p prodex-quota` (115 tests).
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::quota_profile_panel::tests` (five native bridge tests).
- PASS: `mojo run -I mojo/prodex_core mojo/tests/quota_profile_panel_test.mojo`.
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` (vendored dependency warnings remain).
- PASS: `cargo fmt --check`, `rtk npm run docs`, and `git diff --check`.
- PASS: ownership, authority, no-fallback, size and crate-boundary guards, including the crate-boundary self-test.

Canonical production share is 36.98799916151347%: 141,161 reachable Mojo LOC
and 240,479 Rust LOC. The typed native adapter adds Rust code while moving
the panel decisions to Mojo; counting rules and exclusions remain unchanged.

- PASS: `PRODEX_MOJO_VERSION=1.2.0 rtk npm run test:changed` and the staged churn guard.

Next slice: migrate `format_openai_additional_limit_summaries` and
`format_additional_reset_pair` in `crates/prodex-quota/src/render/reports.rs`.
Share the already-compiled additional-name precedence across quota messages,
panels and report summaries while retaining host-local reset-time acquisition.
