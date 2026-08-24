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
10. Provider-independent numeric scoring batch after Rust hard eligibility.
11. Smart Context byte-size token estimate.
12. Strict release/installer metadata and compiled-in Linux artifact gate.
13. Complete governed provider routing-plan batch: Rust validation and policy normalization,
    Mojo capability filtering, bounded score arithmetic, and stable eligible ordering.
14. Provider route capability matching batch over well-formed flags and capability masks.
15. Smart Context pressure snapshot: Rust token/risk normalization plus Mojo capacity,
    pressure-band, safety-floor, and confidence arithmetic.
16. Runtime response candidate execution-plan ordering: Rust hard filtering and plan-model
    reconstruction plus one Mojo batch for ready/fallback ordering.
17. Optimistic current-candidate decision: Rust string normalization plus one Mojo ordered
    predicate result, promoted after 5,000 exact precedence cases.
18. Provider request constraint kernel: Rust JSON/catalog/provider normalization plus one Mojo
    decision covering capability, output-limit, context, reserve, warning, and saturation policy.
19. Smart Context rehydration admission: Rust artifact identity collection and ordering plus
    one Mojo bounded budget/action batch.
20. Quota main-pool aggregation: Rust report/model normalization plus one Mojo bounded numeric
    aggregation batch for Gemini/Copilot main quota rows.
21. Runtime tuning default tuple: one Mojo normalized parallelism batch used by runtime config,
    probe refresh, websocket worker, and async/log default callers.
22. Runtime quota window summaries: active proxy observation/snapshot classification now reuses
    compiled quota status and pressure-band kernels; Rust retains clocks, state, and adapters.
23. Runtime policy numeric validation: one Mojo batch now owns active runtime-proxy numeric
    bounds and governance session range/relation checks; Rust retains security and exact errors.
24. Runtime profile scheduling order: one Mojo index batch now owns normalized ready-profile
    scoring, reserve bias, and ordering; Rust retains clock/state reads, names, and input
    normalization.
25. Critical-signal loss/gain arithmetic: Rust classifies lines and Mojo compares the seven
    normalized counters; Rust retains duplicate matching, range selection, and text handling.

## Next order

1. Measure complete boundaries for the new provider, rehydration, quota, tuning, and optimistic
   kernels including Rust normalization and reconstruction.
2. Revisit the dormant Smart Context candidate scorer/selector only if a non-test production
   caller is restored; do not add an unused Mojo wrapper.
3. Revisit remaining normalized policy numeric rules only after security and error contracts are
   explicit.
4. Reevaluate macOS/ARM64/Windows release rows only after final-link and clean-runtime evidence.

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
- Do not promote a component with parity gaps or an uncertain stream/affinity invariant;
  after promotion, a mismatch is a validation failure and never selects Rust.
