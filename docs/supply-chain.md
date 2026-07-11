# Supply-chain policy

## Rust support

Prodex's minimum supported Rust version (MSRV) is **1.97.0**. The root
`Cargo.toml`, `rust-toolchain.toml`, CI, and the Docker builder use that exact
release. The toolchain file also installs `clippy` and `rustfmt`, so local and
CI checks use the same compiler components.

Review the MSRV monthly and within seven days of a Rust security release.
Upgrade it only after the locked all-feature build, clippy, tests, release
target builds, and Docker build pass together. Record an MSRV increase in the
release notes. Dependabot continues to review Cargo, npm, GitHub Actions, and
Docker updates weekly.

The standalone fuzz workspace uses `cargo-fuzz` 0.13.2 and
`nightly-2026-07-11`. This dated nightly is only for libFuzzer/AddressSanitizer;
it does not change the product MSRV. CI validates `fuzz/Cargo.lock` before
building every fuzz target.

## Immutable inputs

Third-party Actions use full 40-character commit SHAs with the corresponding
major tag in a comment. The tag comment lets Dependabot retain and update the
pin. The current pins were resolved from the upstream repositories' official
refs:

| Input | Readable ref | Commit |
| --- | --- | --- |
| `actions/checkout` | `v7` | `9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0` |
| `actions/setup-node` | `v6` | `48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e` |
| `actions/cache` | `v6` | `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` |
| `actions/upload-artifact` | `v7` | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` |
| `actions/download-artifact` | `v8` | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` |
| `actions/attest-build-provenance` | `v4` | `0f67c3f4856b2e3261c31976d6725780e5e4c373` |
| `dtolnay/rust-toolchain` | `stable` | `4be7066ada62dd38de10e7b70166bc74ed198c30` |
| `Swatinem/rust-cache` | `v2` | `e18b497796c12c097a38f9edb9d0641fb99eee32` |
| `mozilla-actions/sccache-action` | `v0.0.10` | `9e7fa8a12102821edf02ca5dbea1acd0f89a2696` |

Docker Official Image manifest-list digests were resolved from the registry
with `docker buildx imagetools inspect`. The pinned Rust, Debian, PostgreSQL,
and Redis indexes include both Linux amd64 and arm64 manifests. Syft and
Gitleaks CI images are also tag-and-digest pinned. Dependabot owns Dockerfile
and Compose refreshes. Kubernetes uses the immutable digest published for the
selected Prodex release; deployment promotion owns that application-image
update.

Primary pin sources:

- [GitHub Actions repositories](https://github.com/actions)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [mozilla-actions/sccache-action](https://github.com/mozilla-actions/sccache-action)
- [Docker Official Images](https://github.com/docker-library/official-images)
- [Syft](https://github.com/anchore/syft)
- [Gitleaks](https://github.com/gitleaks/gitleaks)

## Required gates

The `supply-chain` CI job runs locked clippy, `cargo audit`, all configured
`cargo deny` checks, pinned `cargo-machete 0.9.2`, and source SBOM generation.
`deny.toml` allows only the licenses present in the reviewed lockfile, denies
wildcard dependencies and OpenSSL/native-tls, and treats duplicate versions as
errors. Every duplicate exception names one exact older version, its current
transitive owner, and its removal condition.

The release workflow:

1. builds with `Cargo.lock` and `--locked`;
2. attests every binary and the SPDX JSON SBOM;
3. downloads the staged assets and verifies their GitHub attestations;
4. generates and verifies `SHA256SUMS`; and
5. publishes the binaries, SBOM, and checksum file together.

Run the local policy checks with:

```bash
npm run ci:supply-chain-guard
npm run ci:secret-boundary-guard
cargo deny check advisories bans licenses sources
cargo machete --with-metadata
cargo +nightly-2026-07-11 fuzz build --fuzz-dir fuzz
```

The secret boundary guard scans production Rust/CLI sources and documentation
while recognizing test/redaction fixture regions. It rejects new
secret-bearing CLI flags and capabilities interpolated into URL queries,
paths, or userinfo. Four existing provider/gateway compatibility flag sites
remain as a fixed, non-growing budget until their public migration is planned.

Production gateway workloads and the external PostgreSQL migrator resolve `SecretRef` values from
projected files under `/run/secrets/prodex`; the migration Job does not inject its database URL
through `envFrom`. The live control-plane workload has a dedicated typed policy and a separate
ExternalSecret limited to its admin token plus PostgreSQL and Redis references. Neither provider
credentials nor data-plane bearer tokens enter that pod, and no shell wrapper copies projected
file contents back into the process environment.
