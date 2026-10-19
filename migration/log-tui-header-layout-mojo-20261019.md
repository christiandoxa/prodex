# Compiled log TUI header and quota layout — 2026-10-19

The production log_semantics.mojo module now owns deterministic header and
quota layout plans under prodex_mojo_log_tui_header_layout_v1. Three versioned
operations decide which parts may fit in the available terminal width:

- Header/title/throughput segmentation, including throughput-only and
  clipped-title modes and space allocated for optional header detail.
- Atomic quota-suffix display, with either a full or a middle-ellipsized
  profile name; an insufficient-width suffix is omitted entirely.
- Saturating remaining whitespace after the Rust host has rendered the
  measured Unicode-aware left and right header segments.

The host must measure actual terminal display widths using terminal_ui,
format the existing quota and rate strings, and render exactly the typed plan.
The previous Rust header width decisions, quota fitting branch tree and
right-alignment arithmetic have been deleted. There is no alternate Rust
policy path or fallback when the compiled Mojo ABI fails.

Validation from the repository root:

    mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_tui_header_layout_test.mojo
    PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -p prodex-mojo-core --features mojo-core --lib log::tui::tests
    PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -p prodex-app --lib app_commands::log_tui::tests
    node scripts/ci/mojo-no-fallback-guard.mjs --self-test
    node scripts/ci/mojo-production-share.mjs --check --json

Direct ABI regressions exercise empty widths, truncation boundaries, extreme
counts, gap saturation and invalid output-pointer/operation/flag handling.
The real application test checks Unicode profile widths, atomic quota suffixes,
and right-alignment. Static guards prohibit reintroducing Rust layout formulas.

This remains an incremental measured production migration; 75% ownership is
not yet reached.

Checkpoint validation: the direct Mojo test, 7 compiled TUI ABI tests,
17 application TUI tests, formatting, Markdown lint, no-fallback self-test
and source guard, and production-share release/non-regression checks pass.
The real caller regression lives in log_tui_header_tests.rs to avoid growing
the live TUI module past the cohesion threshold.

The official source share changes from 35.173561937323846% at 5c1b97896
to 35.172814243880104% (125,995 Mojo / 358,217 production lines).
The typed bridge costs more Rust lines than this small semantic extraction
removes; this checkpoint makes no source-share improvement claim.

`npm run test:changed` fails on pre-existing size violations: the 860-line
deepseek_sse_tests.rs exceeds the 850 production-file limit, and the
repository exceeds its 32-file near-limit budget. Workspace Clippy fails
on pre-existing chunks_exact_to_as_chunks in anthropic_stream_policy.rs
and items_after_test_module in provider_constraints/gemini_bridge_request.rs.
No guard was weakened. Full CI and native macOS/Windows execution were not run.
