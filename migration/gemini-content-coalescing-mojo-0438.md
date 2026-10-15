# Gemini content coalescing: compiled Mojo authority

The Gemini hardening pipeline now delegates content coalescing to
`prodex_mojo_gemini_content_hardening_v1` (ABI 1, operation 0).
Mojo filters missing/invalid/empty parts, supplies the existing default
role, groups adjacent equal roles and retains the first content metadata.
All part values, including primitive JSON and Unicode strings, retain order.

The old Rust loop was removed. Rust only builds the existing borrowed
Serde tree, allocates the measured output, decodes JSON and replaces the
content vector after a complete result. There is no Rust recomputation.

Validation with `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0`:
- Provider-core library: 441 tests passed, including 3 new caller cases.
- Mojo-core direct ABI filter `gemini_content_abi`: 2 passed.
- Provider-core all-target Clippy with warnings denied: passed.
- Ownership, authority, no-fallback, production-share and size guards: passed.

Broad production-source share: 33.19% (110,811 Mojo / 333,898 total LOC).
The 75% project target is not yet met; version/release metadata is unchanged.
