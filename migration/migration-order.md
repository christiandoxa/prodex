# Migration order

## Completed slice

1. Baseline workspace build and test compilation.
2. Repository/module inventory.
3. Local Mojo compiler and official capability audit.
4. Scalar Rust-to-Mojo C ABI spike.
5. `prodex-quota::render::remaining_percent` port with differential parity.
6. Quota window status and pressure-band policy port.
7. Batched quota window-pair readiness port.
8. Runtime route-specific quota pressure-band decision port.
9. Shared `prodex-mojo-core` archive and target-aware build path.
10. Runtime profile quota scoring batch with saturation parity.
11. Provider-independent numeric scoring batch after Rust hard eligibility.
12. Smart Context byte-size token estimate.
13. Strict release/installer metadata and compiled-in Linux artifact gate.
14. Complete governed provider routing-plan batch: Rust validation and policy normalization,
    Mojo capability filtering, bounded score arithmetic, and stable eligible ordering.
15. Provider route capability matching batch over well-formed flags and capability masks.
16. Smart Context pressure snapshot: Rust token/risk normalization plus Mojo capacity,
    pressure-band, safety-floor, and confidence arithmetic.
17. Runtime response candidate execution-plan ordering: Rust hard filtering and plan-model
    reconstruction plus one Mojo batch for ready/fallback ordering.

## Next order

1. Measure complete routing-plan and capability-matching workloads including FFI conversion
   and Rust-side plan reconstruction.
2. Revisit the dormant Smart Context candidate scorer/selector only if a non-test production
   caller is restored; do not add an unused Mojo wrapper.
3. Reevaluate provider request constraints only after parsing, catalog, and error reconstruction
   are isolated from the normalized numeric kernel.
4. Revisit normalized policy validation only after security and error contracts are explicit.
5. Reevaluate macOS/ARM64/Windows release rows only after final-link and clean-runtime evidence.

The generic domain `commit_reservation` and `evaluate_rate_limit` helpers remain Rust-only:
the former is not the production durable accounting path, and distributed rate limiting is
owned by the Redis/runtime adapters. Do not add a Mojo wrapper without a real production seam.

## Stop conditions

- Do not migrate Tokio, Hyper, Reqwest, rustls, database drivers, OAuth, keyring, PTY,
  subprocess supervision, terminal integration, or provider serialization for language
  percentage reasons.
- Do not expose Rust-owned heap graphs, `String`, `Vec`, maps, secrets, or persistent
  state across FFI until their layout and ownership contracts are independently verified.
- Do not add a Mojo call inside a hot loop; batch the entire dataset first.
- Do not enable a component after parity gaps or after a stream/affinity invariant is
  uncertain.
