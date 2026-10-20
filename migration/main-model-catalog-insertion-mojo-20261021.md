# Main Model Catalog Insertion Placement Migrated to Mojo (2026-10-21)

## Production ownership

The existing compiled rich_catalog.mojo owner now chooses the
insertion index for the interactive main-model choice list through
the versioned prodex_mojo_rich_catalog_insertion_index_v1 ABI.

The kernel recognizes original typed choice tags and places a
remembered/current model before the first Custom choice (or
at the end when Custom is absent). It places bundled OpenAI models
immediately after the first ProviderDefault choice (or at the
start when ProviderDefault is absent). All tags are validated
before output mutation and first-match precedence is deterministic.

Rust retains provider catalog acquisition, model selection, effort
metadata, choices and Vec insert/splice materialization. Both
previous Rust .position searches that independently determined
insertion positions were deleted without a semantic fallback.

## Verification

- All 16 prodex-app super_main_prompt tests pass.
- Two additional live main-catalog caller regressions pass,
  covering both insertion modes, missing choices and Unicode
  model labels.
- Two compiled-core tests pass for first-match placement,
  repeated tags, 1,025 choices, invalid later tags, bad ABI
  versions and unchanged rejected output.
- Direct Mojo ABI regression, source no-fallback guard/self-test,
  Cargo formatting and official production-share non-regression
  validation pass.

Official share rises from 36.091252613612696% to
36.09407450674974% (133,473 Mojo / 236,319 Rust production LOC).
The 75% objective remains unmet. Source counting and historical
release floors remain unchanged; remote CI green is not required.
