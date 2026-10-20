# Gemini exact command-output conversation selection (2026-10-20)

## Ownership change

The production Gemini guardrail conversation module now implements the exact
command-output selection as its third versioned JSON ABI operation. Mojo finds
the last user message and the latest subsequent tool message, classifies the
output-only instruction, extracts the required command, and extracts the last
tool's payload text. It uses the existing compiled text validators and the
iterative, first-present-field payload walker.

Rust retains only Serde wire acquisition and typed plan decoding, bounds checks
on selected message offsets, and application of the separately compiled
command/verification-marker match. The old Rust last-user/last-tool selection,
output-only classifier wrapper, required-command extraction wrapper, and
tool-output collector have been deleted. The retained command-candidate
acquisition remains scoped for a separate migration.

## Evidence and limitations

The production user/tool tests cover last-user precedence, a missing tool,
the latest-tool precedence, explicit null payload precedence, Unicode and
nested payloads, command-argument JSON strings, direct assistant args,
exact command mismatches, marker case sensitivity, and blocked success notices.

A direct ABI test exercises the new JSON result, exact-command extraction,
Unicode, invalid message-tree shapes, and ABI version/operation failures before
output metadata writes. The existing Mojo-core compiler remains required;
there is no production Rust fallback for the migrated conversation selection.

Focused verification:
- PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-provider-core --features mojo --lib gemini_bridge::tooling::guardrails::tests -- --test-threads=1
- PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib gemini_guardrails::conversation_tests -- --test-threads=1
- mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_guardrail_conversation_test.mojo
- node scripts/ci/mojo-no-fallback-guard.mjs --self-test
- node scripts/ci/mojo-production-share.mjs --check --json

The canonical broad production share rises from 35.695920371657785% to
35.714305415730024% (129,484 Mojo lines and 233,071 Rust lines).
The 75% project target is not met. No counting, baseline, release floor, or
non-regression policy was weakened to compensate for the incremental migration.
