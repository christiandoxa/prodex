# Runtime-doctor binding source aggregation migrated to Mojo

The runtime-doctor binding-state diagnostics now use
`prodex_mojo_runtime_doctor_binding_source_v1`, a compiled-in, versioned
Mojo reducer. Production Mojo owns per-kind and per-profile binding totals,
oldest/newest signed timestamps, the first eight missing-profile sample
indices, and descending profile-total ordering with lexical-name ties.

Rust scans its owned source maps, resolves borrowed profile identifiers and
known-profile membership, and assigns lexical profile-name indexes. It
passes fixed-width rows to the ABI, validates and materializes Mojo results
into the existing diagnostic DTOs, and formats missing-binding examples.
The old Rust reducer, sample-limit branching, and sort closure were
deleted; there is no Rust semantic fallback or duplicate production owner.

The ABI validates counts, row profile indexes, kind tags, known flags,
and required pointers before emitting output; the typed adapter validates
the returned permutation, profile totals, and source sample indexes.
Profile ordering uses an in-place, stable, scratch-buffered Mojo merge sort
instead of a potentially quadratic rank pass. Empty inputs preserve empty
profile lists and absent timestamps.

## Strict local verification

- Real Mojo-core ABI tests: 3 passed. Coverage includes all binding kinds,
  negative timestamps, missing samples, ties, descending rank, invalid row,
  version mismatch, and empty input.
- Required-Mojo runtime-doctor complete library suite: 51 passed,
  including the active binding caller and first-eight missing-profile
  order, signed timestamp bounds, descending totals and tie ordering.
- `node scripts/ci/mojo-ownership.mjs --check`: PASS.
- `node scripts/ci/mojo-authority-guard.mjs`: PASS.
- `node scripts/ci/mojo-no-fallback-guard.mjs --check`: PASS.
- `node scripts/ci/mojo-production-share.mjs --check`: PASS.

Broad production source is 32.95% Mojo (109,046 / 330,898 LOC).
The 75% project target is still outstanding, and no CI-green gate was
awaited for this checkpoint.
