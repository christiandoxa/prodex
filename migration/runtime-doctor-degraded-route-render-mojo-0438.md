# Runtime-doctor degraded-route line rendering migrated to Mojo

The compiled `runtime_doctor_degraded_routes.mojo` domain now owns the
`prodex_mojo_runtime_doctor_degraded_route_render_v1` operation. It formats
each Mojo-selected degraded-route record: Unicode profile/route text,
implicit retry/transport labels, circuit `open` versus `half-open`
status, all five kind-specific suffixes and signed decimal timestamps or
scores, including `i64::MIN` and `i64::MAX`.

The prior production Rust formatter matched kinds and fallback routes
and used several `format!` branches; that implementation was removed.
Rust still owns BTreeMap and score ingestion, checked source indices,
UTF-8 slice boundaries, Mojo ABI marshalling and verified String output.
No Rust fallback renderer or duplicate semantic owner remains.

## Local verification

- Required-Mojo direct formatter ABI tests: 2 passed, covering all
  five kinds, implied and explicit routes, Unicode, negative timestamps,
  extremal signed values, circuit transition and invalid ABI inputs.
- Required-Mojo caller tests: three degraded-route cases passed,
  including actual map selection/decay/top-eight ordering and extra
  Unicode/negative/maximum-value regression.
- Full required-Mojo runtime-doctor library suite: 57 passed.
- Clippy with `--all-targets -- -D warnings`: PASS.
- Mojo ownership, authority, no-fallback and production share: PASS.
- Broad production Mojo share: 33.45%
  (112,138 Mojo LOC of 335,279 total LOC). Target 75% not yet met.
