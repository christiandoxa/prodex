# Gemini Apply-Patch Source Selection Migrated to Mojo (2026-10-20)

## Production ownership

The existing Gemini tooling owner now chooses the first string from
the explicit patch aliases, the first pathname alias with Unicode
whitespace trimming and empty-first-string blocking, the first old/new
string aliases, and the optional content fallback through the versioned
prodex_gemini_apply_patch_sources_v1 ABI.

The Rust host only borrows the 17 typed Serde fields and applies
validated Mojo-selected indexes and UTF-8 byte slices to the already
compiled patch normalization, unified-diff, edit and replacement
formatters. The old Rust explicit/content selector and edit-arguments
lookup functions were deleted, with no Rust semantic fallback.
A non-string field remains absent; a present empty string still blocks
later aliases in the same precedence group.

## Validation

- All 534 strict prodex-provider-core tests pass, including new
  end-to-end tests for empty explicit input, non-string fields,
  Unicode-trimmed filename and blank-path precedence barriers.
- Two direct core ABI tests pass, covering exact field priority,
  original borrowed spans, empty first-string behavior, malformed
  late field tags and untouched output on rejected ABI requests.
- Direct Mojo test and anti-fallback guard/self-test, Cargo formatting
  and official production-share non-regression checks pass.

Canonical production share at the validated working tree is
35.88328851577359% (131,307 Mojo / 234,621 Rust LOC).
The unchanged figure already included the uncommitted WIP at the
previous measurement. Project goal 75% remains unmet; metric
definitions and release floors were not modified. Remote CI green
was not required for this checkpoint.
