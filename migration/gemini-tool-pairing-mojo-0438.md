# Gemini missing-result and orphan-call repair in Mojo

The existing compiled content-hardening ABI now owns operation 1 (missing
result pairing) and operation 2 (orphan response repair). Both use decoded
Serde identities: id and name must match, duplicate unmatched entries stay
ordered, and original metadata/parts remain intact. Explicit synthetic
results still say that execution output is unavailable; no tool executes.

Both former Rust matching/synthesis loops and the obsolete response
collector were removed. Rust retains only the existing JSON acquisition,
measured output allocation and complete-array decoding adapter. The
pre-existing Mojo tool-response ordering policy remains unchanged.

Required-Mojo validation (`PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0`):
- Complete provider-core library: 445 tests passed.
- Direct content ABI tests: 2 passed, including all three empty-input modes.
- Provider-core all-target Clippy with denied warnings: passed.
- 2,048 generated complete-pipeline cases equal the source at `2a6d419bf`.
  The temporary reference module was removed after testing, not committed.
- Ownership and authority checks passed on the exact staged Git tree using
  their canonical exported validators; unrelated concurrent work was excluded.
- No-fallback and size guards passed.

This batch's isolated production snapshot is 33.23% Mojo
(110,938 / 333,894 LOC). The project target remains 75%; no counting rules,
release version or baseline were changed.
