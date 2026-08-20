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

## Next order

1. Measure a complete batch quota/routing workload including FFI conversion.
2. Evaluate provider-independent candidate filtering as a batch algorithm; keep hard
   affinity, authorization, and runtime state in Rust.
3. Evaluate Smart Context candidate scoring as a batch algorithm; keep tokenizer and
   input collection in Rust.
4. Revisit normalized policy validation only after security and error contracts are explicit.
5. Evaluate accounting/budget arithmetic only as a candidate result; Rust retains enforcement.
6. Reevaluate macOS/ARM64/Windows release rows only after final-link and clean-runtime evidence.

## Stop conditions

- Do not migrate Tokio, Hyper, Reqwest, rustls, database drivers, OAuth, keyring, PTY,
  subprocess supervision, terminal integration, or provider serialization for language
  percentage reasons.
- Do not expose Rust-owned heap graphs, `String`, `Vec`, maps, secrets, or persistent
  state across FFI until their layout and ownership contracts are independently verified.
- Do not add a Mojo call inside a hot loop; batch the entire dataset first.
- Do not enable a component after parity gaps or after a stream/affinity invariant is
  uncertain.
