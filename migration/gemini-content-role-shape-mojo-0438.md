# Gemini content role-boundary repair in Mojo

Operation 3 of `prodex_mojo_gemini_content_hardening_v1` now owns the
leading/trailing user-message decisions and final same-role coalescing.
The original Rust role guards and synthetic-message assembly are removed.
The existing JSON adapter preserves the public hardening API and mutates
the caller vector only after a complete valid Mojo result is available.

Boundary handling preserves the original order of operations: decisions
use the raw first/last role before invalid rows are removed, synthetic
leading metadata takes precedence, and trailing parts merge into a final
user group. Unknown roles are not silently converted into model roles.

Required-Mojo validation:
- Provider-core full library: 447 passed, including 11 hardening cases.
- Direct content ABI tests: 2 passed, exercising all four operation modes.
- Complete pipeline against source `2a6d419bf`: 2,048 generated cases agree.
- Provider-core all-target Clippy with warnings denied: passed.
- Ownership, authority, no-fallback, production-share and size guards: passed.

The shared branch snapshot is 33.25% Mojo (111,082 / 334,051 LOC), also
including a separately committed concurrent smart-context checkpoint.
The 75% target is outstanding; release metadata and counting rules are unchanged.
