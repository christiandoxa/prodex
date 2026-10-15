# Runtime-doctor diagnostic text formatting migrated to Mojo

The existing compiled runtime-doctor renderer now owns two additional
production-reachable operations:

- `prodex_mojo_runtime_doctor_scope_v1`: optional profile/route scope
  presence, slash joining and correct distinction between absent and empty.
- `prodex_mojo_runtime_doctor_count_breakdown_v1`: exact
  `label=count, ...` formatting, including Unicode labels, zero values,
  maximum unsigned counts, and the empty-map `-` sentinel.

The Rust diagnostic marker-accessor functions retain BTreeMap lookup
and ordered borrowed input materialization, but no longer choose scope
layout, digit formatting, or separators. The new ABI functions reuse
the existing bounded Mojo writer and reject invalid pointers, presence
flags, unrepresentable source counts and short output capacity.
They do not invoke a Rust formatter fallback.

## Required-Mojo verification

- Direct Mojo renderer regression tests: 3 passed, including all profile
  and route presence permutations and invalid ABI boundaries.
- Real runtime-doctor caller regression: 1 passed.
- Full runtime-doctor library suite: 55 passed.
- Clippy with `--all-targets -- -D warnings`: pass.
- Ownership, authority, no-fallback and production-share guards: PASS.
- Mojo share now 33.17% (110,547 / 333,304 production source LOC).
  The project target of 75% is not yet met.
