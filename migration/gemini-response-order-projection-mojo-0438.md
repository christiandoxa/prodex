# Gemini response-order JSON projection in Mojo

Operation 4 of the compiled content-hardening ABI now owns neighbour-role
eligibility, optional call/response id extraction and JSON materialization
in the selected order. The existing provider-constraints permutation
algorithm is reused through one non-exported Mojo helper; its original C
ABI stays compatible. No second sorting policy or Rust fallback was added.

The obsolete Rust part-inspection module and the response-order projection
loop were removed. Rust retains Serde acquisition, checked scratch/output
allocation, complete JSON decoding and same-length result assignment. The
scratch region is sized explicitly for the shared sorter's typed records.

Validation with required Mojo:
- Complete provider-core library: 448 tests passed.
- Mojo-core Gemini-related ABI/kernel tests: 26 passed.
- Full pipeline versus baseline source at `2a6d419bf`: 2,048 cases matched.
- Provider-core all-target Clippy with denied warnings: passed.
- Ownership and authority checks pass on the exact staged source snapshot.
- No-fallback guard now follows each new hop and still rejects restored Rust
  sorting; negative self-tests pass. Size and offline compatibility gates pass.

The isolated snapshot measures 33.27% Mojo (111,177 / 334,118 LOC).
Concurrent unrelated work is not part of this checkpoint. No release,
counting-rule, baseline or target metadata is changed; 75% remains pending.
