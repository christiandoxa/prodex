# Runtime admission affinity uses Mojo authority

Responses and WebSocket lane-cap bypass now use the required
`prodex_runtime_proxy_admission_affinity_v1` Mojo policy. Rust still reads
continuation bindings under the runtime lock and keeps compact lineage lookup,
atomic admission, waiting, logging, and all other host effects. The former Rust
binding-or decision is gone; the typed adapter rejects invalid ABI output without
recomputing the decision in Rust.

Compiled ABI vectors and the existing runtime admission caller tests cover
bound and unbound continuations across all four lanes.
