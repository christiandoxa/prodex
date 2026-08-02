# Supply-chain policy

## Rust support

Prodex's minimum supported Rust version (MSRV) is **1.97.1**. The root
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

The npm wrapper pins Codex to the exact release recorded in
`npm/prodex/lib/codex-compat.cjs`; release metadata generation derives the main
dependency and every platform alias from it. `package-lock.json` records the
registry integrity values. Opt-in `PRODEX_CODEX_AUTO_INSTALL` installs that
exact version and rejects a post-install `codex --version` mismatch. Update the
canonical file, the standalone installer marker, and the lockfile together,
then run `npm run npm:sync-version`, `npm ci`, and
`npm run ci:supply-chain-guard`.

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
| `SonarSource/sonarqube-scan-action` | `v8.1.0` | `7006c4492b2e0ee0f816d36501671557c97f5995` |

Docker Official Image manifest-list digests were resolved from the registry
with `docker buildx imagetools inspect`. The pinned Rust, Debian, PostgreSQL,
and Redis indexes include both Linux amd64 and arm64 manifests. Syft, Gitleaks,
and KICS CI images are also tag-and-digest pinned. The Rust quality job uses
SonarQube Community Build `26.7.0.124771-community` at manifest digest
`sha256:160bd2f6a3485bd09b655ef22dd63c02bd1fa7ba82aa5d9973fd010b8bcca0b3`.
The KICS gate uses `v2.1.20` at manifest digest
`sha256:3e5a268eb8adda2e5a483c9359ddfc4cd520ab856a7076dc0b1d8784a37e2602`.
Dependabot owns Dockerfile and Compose refreshes. The release workflow scans
the locally built image with digest-pinned Trivy 0.72.0, failing on fixable
high/critical vulnerabilities, then publishes an attested GHCR image and
renders the Kubernetes release asset from that exact registry digest. The
checked-in manifest keeps a non-deployable digest placeholder so an old digest
cannot be mistaken for the current release.

Primary pin sources:

- [GitHub Actions repositories](https://github.com/actions)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [mozilla-actions/sccache-action](https://github.com/mozilla-actions/sccache-action)
- [SonarQube Docker Official Image](https://hub.docker.com/_/sonarqube)
- [SonarQube for VS Code supported languages](https://docs.sonarsource.com/sonarqube-for-vs-code/using/rules/)
- [Docker Official Images](https://github.com/docker-library/official-images)
- [Syft](https://github.com/anchore/syft)
- [Gitleaks](https://github.com/gitleaks/gitleaks)
- [KICS](https://github.com/Checkmarx/kics)

## Required gates

The `rust-quality`, `kics`, and `supply-chain` jobs run in parallel. The first
two run for every commit. `rust-quality`
generates the production-only Clippy JSON report, enforces zero Clippy warnings
across all targets, and imports that report into a job-local SonarQube Community
Build instance. Its token is generated inside the ephemeral runner, masked,
and revoked before the job exits; no Sonar repository secret or variable is
required. The production report uses `cargo clippy --locked --workspace
--exclude prodex-bench-support --lib --bins --all-features
--message-format=json -- -D warnings` and writes the ignored
`target/sonar/clippy-report.json`.

Sonar scans only Rust under `src` and `crates`, excluding dedicated test
modules and directories, fixtures, test support, generated/vendor/build
content, and `crates/prodex-bench-support`; production runtime self-test code
remains indexed. CI requires both an `OK` quality gate and zero unresolved
issues. SonarQube for VS Code is not used as this gate: its official language
list does not include Rust, and its analysis is editor-triggered rather than a
deterministic headless CI interface.

The `kics` job scans the checked-in Dockerfile, Compose file, Kubernetes
manifests, and GitHub workflows through a read-only repository mount. Any high
or critical infrastructure finding fails CI. KICS secret heuristics are disabled
because the separate pinned Gitleaks job owns secret detection; other KICS
queries remain enabled. The scanner container has no network, capabilities, or
writable root filesystem. The only inline suppression covers a read-only
projected Kubernetes Secret volume that KICS otherwise mistakes for a writable
host mount.

The parallel `supply-chain` job runs `cargo audit`, all configured `cargo deny`
checks, pinned `cargo-machete 0.9.2`, and source SBOM generation.
`deny.toml` allows only the licenses present in the reviewed lockfile, denies
wildcard dependencies and OpenSSL/native-tls, and treats duplicate versions as
errors. Every duplicate exception names one exact older version, its current
transitive owner, and its removal condition.

The release workflow:

1. builds with `Cargo.lock` and `--locked`;
2. attests every binary and the SPDX JSON SBOM;
3. downloads the staged assets and verifies their GitHub attestations;
4. generates and verifies `SHA256SUMS`; and
5. scans and attests the GHCR image, renders the Kubernetes manifest with its
   registry digest, and publishes that manifest plus the vulnerability report
   with the binaries, SBOM, and checksum file.

Run the local policy checks with:

```bash
npm run ci:supply-chain-guard
npm run ci:secret-boundary-guard
mkdir -p target/kics
docker run --rm \
  --user "$(id -u):$(id -g)" \
  --read-only --cap-drop ALL --security-opt no-new-privileges:true --network none \
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  --volume "${PWD}:/path:ro" \
  --volume "${PWD}/target/kics:/results" \
  docker.io/checkmarx/kics:v2.1.20@sha256:3e5a268eb8adda2e5a483c9359ddfc4cd520ab856a7076dc0b1d8784a37e2602 \
  scan -p /path -o /results --output-name prodex-kics --report-formats json,sarif \
  --disable-secrets --disable-full-descriptions --fail-on critical,high \
  --minimal-ui --no-progress
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
ExternalSecret limited to distinct maker/checker admin credentials, native mTLS server/probe
material, and PostgreSQL/Redis references. Neither provider credentials nor data-plane bearer
tokens enter that pod, and no shell wrapper copies projected file contents back into the process
environment.
