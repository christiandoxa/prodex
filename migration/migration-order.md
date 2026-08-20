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

## Next order

1. Measure a batch quota calculation before adding more one-value FFI calls.
2. If useful, extract Rust-only quota fraction/amount conversion and port the complete
   calculation as one coarse-grained function.
3. Evaluate provider-independent candidate filtering as a batch algorithm; keep hard
   affinity and runtime state in Rust.
4. Evaluate Smart Context scoring as a batch algorithm; keep tokenizer and input
   collection in Rust.
5. Extract provider-independent routing scores from hard eligibility and affinity, then
   run offline differential fixtures.
6. Revisit policy validation only after security and error contracts are explicit.
7. Reevaluate the matrix after a Mojo release changes string, collection, serialization,
   or platform support.

## Stop conditions

- Do not migrate Tokio, Hyper, Reqwest, rustls, database drivers, OAuth, keyring, PTY,
  subprocess supervision, terminal integration, or provider serialization for language
  percentage reasons.
- Do not expose Rust-owned heap graphs, `String`, `Vec`, maps, secrets, or persistent
  state across FFI until their layout and ownership contracts are independently verified.
- Do not add a Mojo call inside a hot loop; batch the entire dataset first.
- Do not enable a component after parity gaps or after a stream/affinity invariant is
  uncertain.
