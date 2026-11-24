# Supply-chain policy

## Rust support

Prodex's minimum supported Rust version (MSRV) is **1.100.0**. The root
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

CI also pins Node.js **26.13.1**, npm **12.2.3**, uv **0.13.9**,
Python **3.15.2**, and Mojo **1.2.0**. Each Node setup explicitly installs
the pinned npm version because the Node distribution bundles its own npm
release. The sccache Action selects **0.20.0** through its explicit version
input; the Action commit and the installed tool are pinned independently.

## Immutable inputs

Release packages contain only Prodex executables and metadata. They do not
depend on, download, compile, bundle, or publish Codex. Standalone and npm
launches resolve the official Codex CLI from `PATH`, or from
`PRODEX_CODEX_BIN`, reject Prodex/npm wrapper recursion and the exact six
Codex binaries previously shipped by Prodex 0.426.1, and require Codex 0.153.2
or newer with the `app-server` command. Legacy bundled files remain untouched.
Install Codex from
the [official Codex CLI instructions](https://developers.openai.com/codex/cli)
before launching a Codex-backed Prodex command.

The release workflow attests and checksums only Prodex executables alongside
release metadata. Its SBOM input is restricted to those executables, and the
final asset gate rejects Codex-named files. Run
`node scripts/ci/supply-chain-guard.mjs --self-test` to check the packaging
boundary.

Third-party Actions use full 40-character commit SHAs with the corresponding
major tag in a comment. The tag comment lets Dependabot retain and update the
pin. The current pins were resolved from the upstream repositories' official
refs:

| Input | Readable ref | Commit |
| --- | --- | --- |
| `actions/checkout` | `v7` | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| `actions/setup-node` | `v7` | `949feb2413d6458794dcd2491c4babbbce0c15c1` |
| `actions/cache` | `v6` | `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` |
| `actions/upload-artifact` | `v7` | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |
| `actions/download-artifact` | `v8` | `9000827ccba6bdab643e8b6fd33ac0654aef8333` |
| `actions/attest-build-provenance` | `v4` | `4d101475d8b20a2381f78447822ac1eab6504dd8` |
| `dtolnay/rust-toolchain` | `stable` | `39008827e6b4319a349e9a42dfc90579dd25f1ca` |
| `astral-sh/setup-uv` | `v10.4.0` | `1db35dd8eee11c5894872e0b383ae6c406cf738d` |
| `Swatinem/rust-cache` | `v2` | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
| `mozilla-actions/sccache-action` | `v0.0.11` | `fc920bf0ec8de6ee65d409111f7ec508035751ba` |
| `SonarSource/sonarqube-scan-action` | `v8.5.1` | `70bad232163b09d998ea0707ee80c0ec80798c16` |

Docker Official Image manifest-list digests were resolved from the registry
with `docker buildx imagetools inspect`. The pinned Rust, Debian, PostgreSQL,
and Redis indexes include both Linux amd64 and arm64 manifests. Syft, Gitleaks,
and KICS CI images are also tag-and-digest pinned. The Rust quality job uses
SonarQube Community Build `26.11.0.134175-community` at manifest digest
`sha256:7b2296713a92b8ea0dd7a083a25faadc59376c9a1e6daf2744a89711c54ea1ba`.
The KICS gate uses `v2.2.1` at manifest digest
`sha256:49ccc54b6ae7f5d5fc95e96f8f95c18c57f3bba047ab611a03d80792bdff6757`.
The release scan uses the official ClamAV image
`docker.io/clamav/clamav:1.5.5@sha256:64e30c0bf976198024deb3dc6d5cfe2485fb969c0f73549ba0fdb364b9469614`.
Dependabot owns Dockerfile and Compose refreshes. The supply-chain guard keeps
container inputs pinned, while standalone releases publish binaries,
installers, the SBOM, and checksums only; they do not build or publish a
deployment container image or rendered Kubernetes release asset. The
checked-in manifest keeps a non-deployable digest placeholder, so an operator
must substitute the digest from the separately governed deployment image
before applying it.

Primary pin sources:

- [GitHub Actions repositories](https://github.com/actions)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain)
- [astral-sh/setup-uv](https://github.com/astral-sh/setup-uv)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
- [mozilla-actions/sccache-action](https://github.com/mozilla-actions/sccache-action)
- [Official ClamAV container images](https://docs.clamav.net/manual/Installing/Docker.html)
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
--exclude prodex-bench-support --lib --bins
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
manifests, and GitHub workflows through a read-only repository mount. Any
finding at critical, high, medium, low, or info severity fails CI. KICS secret
heuristics are disabled because the separate pinned Gitleaks job owns secret
detection; other KICS queries remain enabled. The scanner container has no
network, capabilities, or writable root filesystem, and the IaC sources contain
no inline KICS suppressions or broad exclusions. The pinned KICS engine has two
reviewed INFO-only policies that do not model these manifests correctly: one
always emits a review reminder for any namespaced workload, and one labels each
declared Compose named volume as shared even when exactly one service mounts it.
CI excludes only their exact query IDs:

- `e84eaf4d-2f45-47b2-abe8-e581b06deb66` (`Ensure Administrative Boundaries Between Resources`)
- `8c978947-0ff6-485c-b0c2-0bfca6026466` (`Shared Volumes Between Containers`)

Every other query remains enabled. CI also reads the JSON report and requires
`total_counter` to equal zero, so a TRACE finding cannot bypass the severity
exit-code gate.

The parallel `supply-chain` job runs pinned `cargo-audit 0.23.0`, all configured
`cargo-deny 0.22.0` checks, `cargo-machete 0.10.1`, and source SBOM generation
with Syft **1.57.2**.
`deny.toml` allows only the licenses present in the reviewed lockfile, denies
wildcard dependencies and OpenSSL/native-tls, and treats duplicate versions as
errors. Every duplicate exception names one exact older version, its current
transitive owner, and its removal condition.

The release workflow:

1. requires an exact `main` commit and version, then verifies CI for that commit;
2. builds with read-only permissions, `Cargo.lock`, and `--locked`;
3. attests every binary in checkout-free jobs and attests the SPDX JSON SBOM;
4. downloads the staged assets and verifies their GitHub attestations;
5. generates, verifies, and attests `SHA256SUMS`; and
6. fetches signed ClamAV definitions, proves the engine detects EICAR, and
   scans the final assets before the tag and public release are created.

Windows x64 uses reproducible PE metadata, and the workflow verifies the
executable's SHA-256 after smoke tests. For `0.171.2`, it compiles from locked
inputs without restoring compiled Rust dependencies. It links the immutable
Mojo archive from the two reviewed builds after verifying the archive SHA-256
and that the source tree differs only in release policy files.

The `0.171.2` Windows x64 executable has one reviewed ClamAV finding,
`Win.Trojan.Virut-225`. Static comparison against `0.171.1` traced
the match to relocated operands across the same three functions, with matching
heap imports and jump table targets. The scan gate accepts this single result
for either of these independently reviewed SHA-256 hashes:

- `10229478760ac554717473cf7771afb5f6c8fdbd83b66fb896a0e8cfd92bfc3f`
- `334d93fb3b399453221c61724c1112b8f6c069d5b57ea854505b1a0c0a120350`

The two builds have identical executable code. Their 118 differing bytes are
AWS-LC short source paths and the derived PE metadata.
A fresh build of the Mojo archive varied the executable code and the gate
rejected the resulting unreviewed hash.
An additional finding, scanner error, or different executable hash fails the
gate. [Verified source build and scan](https://github.com/christiandoxa/prodex/actions/runs/44885409369)

For `0.171.3`, a new source build had the same sole false positive. The 156-byte
match again spanned the same three functions. All 233 normalized instructions
matched `0.171.2`, including their Windows heap imports and switch targets.
The exact reviewed Windows x64 SHA-256 is
`65630e75517dde4bc94e35c042894885776eafc17e7b107ed369a27c099fdf73`.
The workflow verifies source inputs against the original build, then retrieves
its immutable artifact by run and artifact ID. It checks the digest before
runtime smoke tests and scans those bytes with the full release assets.
[Original 0.171.3 build and scan](https://github.com/christiandoxa/prodex/actions/runs/44925191608)

For `0.172.0`, the fresh locked-source build has the same sole finding at raw
file offset `0xFA3E9`. Its exact reviewed Windows x64 SHA-256 is
`97a738a530dd51c0bd1ad2c1dfbaef8864a727f57a596ee850969b747a0f590c`.
Comparison with the reviewed `0.171.3` executable covers 842 bytes across three
functions and their alignment: all 233 non-INT3 instruction records and six
INT3 padding bytes match after normalizing only validated address operands.
The two `86 01` fragments are displacement bytes for `HeapFree` and
`GetProcessHeap`, `66` is an alignment NOP prefix, and the final jump uses a
bounded switch table. Both switch tables and import identities match.
The structure corresponds to `fancy-regex 0.19.2` expression destruction; its
locked package is unchanged. This explains the specific signature collision,
not the safety of every byte or external callee in the executable.

The workflow checks source commit `5a5a93376dc6e3553516be6d772a73c541c91b5e`,
its successful Windows build, immutable artifact `14885229893` from run
`45316006417`, and the artifact ZIP digest before reuse. Source differences are
restricted to the reviewed release policy, documentation and policy tests;
production code, manifests and lockfiles must match. The full executable hash
is checked before and after smoke tests. Fresh official definitions, EICAR,
full scanning, and rejection of additional findings or scanner errors remain
required. [Original 0.172.0 build and finding](https://github.com/christiandoxa/prodex/actions/runs/45316006417)

Run the local policy checks with:

```bash
node scripts/ci/supply-chain-guard.mjs --self-test
node scripts/ci/secret-boundary-guard.mjs --self-test
mkdir -p target/kics
docker run --rm \
  --user "$(id -u):$(id -g)" \
  --read-only --cap-drop ALL --security-opt no-new-privileges:true --network none \
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  --volume "${PWD}:/path:ro" \
  --volume "${PWD}/target/kics:/results" \
  docker.io/checkmarx/kics:v2.2.1@sha256:49ccc54b6ae7f5d5fc95e96f8f95c18c57f3bba047ab611a03d80792bdff6757 \
  scan -p /path -o /results --output-name prodex-kics --report-formats json,sarif \
  --disable-secrets --disable-full-descriptions \
  --exclude-queries e84eaf4d-2f45-47b2-abe8-e581b06deb66,8c978947-0ff6-485c-b0c2-0bfca6026466 \
  --fail-on critical,high,medium,low,info \
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
