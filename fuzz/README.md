# Prodex fuzz targets

The fuzz package is intentionally outside the main workspace so normal workspace builds do not
compile libFuzzer. Install `cargo-fuzz` 0.13.2 and the pinned nightly toolchain, then run:

```bash
cargo +nightly-2026-07-11 fuzz run canonical_request_target --fuzz-dir fuzz
cargo +nightly-2026-07-11 fuzz run oidc_endpoint_policy --fuzz-dir fuzz
cargo +nightly-2026-07-11 fuzz run profile_export_envelope --fuzz-dir fuzz
cargo +nightly-2026-07-11 fuzz run runtime_policy_parse --fuzz-dir fuzz
```

CI validates `fuzz/Cargo.lock` and compiles every target under AddressSanitizer with the same
dated nightly. Local smoke runs can add `-- -max_total_time=10` for a bounded pass.

The request-target harness checks canonical round trips and verifies that owned and borrowed route
classification always agree on the route kind and plane. The OIDC harness exercises issuer, JWKS,
and allowlist combinations while checking canonical issuer identity, same-origin discovery, redirect
denial, and redacted diagnostics.

The profile-export harness caps individual fuzz inputs at 1 MiB for throughput and exercises the
bounded envelope, profile, nested-secret, and secret-file deserializers together.
The runtime-policy harness applies the same input cap, parses only UTF-8 TOML, and runs the
production section validators for every structurally valid policy.
