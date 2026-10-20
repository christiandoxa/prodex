# Gemini Thought-Signature Selection Migrated to Mojo (2026-10-20)

## Production authority and host boundary

The compiled Gemini value policy now owns the case-sensitive Gemini-3
model substring check and extraction of the original thought-signature
text. The existing versioned scalar ABI owns model-family recognition.
The existing versioned JSON-tree ABI owns the first-present
thoughtSignature/thought_signature precedence, Unicode-whitespace
blank check, and preservation of valid original text.

Rust retains Serde JSON acquisition through the borrowed-shape host,
body marshaling, the actual hardening operation, and the returned
Option<String> type. The prior Rust model substring predicate and
thought-signature field/trim fallback chain were deleted. A canonical
field that exists but is null, non-string, or blank blocks the legacy
field. Non-object values still produce no signature.

## Verification

- Mojo source compiles to a production-reachable object.
- Complete strict prodex-provider-core suite: 525 tests pass, including
  model-family and signature-presence caller regressions.
- Focused Gemini value ABI suite: 11 tests pass, including malformed
  ABI version, invalid operation, null inputs, Unicode and original
  whitespace behavior.
- Anti-fallback guard/self-test, Cargo format, diff validation, and
  audited reachable production-share checks pass.

Canonical share increases from 35.78715858637232% to
35.78999741606329% (130,199 Mojo / 233,587 Rust production lines).
The 75% project target remains unmet; source counting, release floors
and non-regression rules have not been changed.
