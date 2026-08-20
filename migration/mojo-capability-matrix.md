# Mojo capability matrix

## Toolchain evidence

| Item | Result |
| --- | --- |
| Local compiler | `Mojo 1.0.0 (ed45d567)` from `mojo --version` |
| Modular CLI | Not installed as a `modular` executable; no command was invented |
| Verified source syntax | `def`, `Int64`, `@export("...")`, and `abi("C")` in `mojo/prodex_core/quota.mojo` |
| Verified artifact | `mojo build ... --emit object --optimization-level=3` produced an x86-64 relocatable object, then `ar` produced a static archive |
| Verified Rust call | Rust linked the object and matched the quota oracle |
| Shared library | `--emit shared-lib` compiled in the spike; not selected for Cargo because it adds runtime loader/distribution state |
| Portability | Target-aware object probes exist for Linux x86_64/aarch64, macOS x86_64/arm64, and Windows COFF; only Linux x86_64 has release link/runtime evidence |
| Real Mojo CI | Ubuntu 24.04 installs official `mojo==1.0.0` with pinned uv `0.11.7`; `PRODEX_MOJO_REQUIRED=1` forbids fallback |

`--emit object` is currently documented as experimental. That is an intentional, narrow
build risk accepted for this scalar slice; the object contains no observed Mojo runtime
dependency. Revisit shared libraries or a supported package/build integration before
shipping broader Mojo components.

| Capability | Needed by Prodex? | Local status | API maturity for this migration | Rust equivalent | Recommendation | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Integers / branches | Yes | Verified | Sufficient for scalar core | Rust integer types | `MOVE NOW` | `mojo/prodex_core/quota.mojo` |
| Floats | Yes, quota fractions/context scores | Available, not tested here | Needs rounding/parity probe | Rust `f32`/`f64` | `EXPERIMENT` | Official Mojo manual/API |
| Strings | Yes | Available, not tested here | Keep boundary in Rust | `String`, `str` | `EXPERIMENT` | Official Mojo manual |
| Collections | Yes | Available, not tested here | Batch-only candidate | `Vec`, `HashMap`, `BTreeMap` | `EXPERIMENT` | Official Mojo stdlib |
| Optionality | Yes | Boundary encoded explicitly | FFI representation verified | `Option<T>` | `MOVE NOW` for scalar pairs | `ffi-contract.md` |
| Error handling | Yes | No FFI errors in slice | Needs explicit tagged contract | `Result<T,E>` | `KEEP RUST` initially | `ffi-contract.md` |
| Traits / generics | Maybe | Not tested | Not needed for first slice | Rust traits/generics | `EXPERIMENT` | Official Mojo manual |
| Filesystem / environment | Yes | Not required by core | Rust is simpler and established | `std::fs`, `std::env` | `KEEP RUST` | Prodex boundary contract |
| JSON / serialization | Yes | Not required by core | Serde is compatibility oracle | Serde / `serde_json` | `KEEP RUST` | Existing provider/config code |
| Hashing / cryptography | Yes | Not required by core | Security-sensitive | Rust crypto crates | `KEEP RUST` | Threat model |
| Time / clocks | Yes | Not required by core | Runtime/timezone behavior matters | `chrono`, `std::time` | `KEEP RUST` | Quota reset paths |
| Randomness | Yes | Not required by core | Security/runtime semantics | `getrandom` | `KEEP RUST` | Existing dependency |
| Threads | Yes | Not required by core | Tokio/process lifecycle owns this | Tokio/std threads | `KEEP RUST` | Runtime policy |
| Async | Yes | Not required by core | No migration need | Tokio | `KEEP RUST` | Runtime policy |
| Networking / HTTP | Yes | Not required by core | Mature Rust stack exists | Hyper/Reqwest | `KEEP RUST` | Architecture |
| TLS | Yes | Not required by core | Security-sensitive ecosystem | rustls | `KEEP RUST` | Threat model |
| Process execution / PTY | Yes | Not required by core | OS-specific | Rust process/PTY crates | `KEEP RUST` | Runtime launch |
| Dynamic libraries / C ABI | Yes | C ABI scalar and flat-buffer exports verified | Narrow stateless boundary only | Rust `extern "C"` | `MOVE NOW` for verified kernels | Mojo export/changelog; `migration/abi_probe.rs` |
| SQLite / PostgreSQL / Redis | Yes | Not required by core | Rust drivers and persistence contracts | rusqlite/postgres/redis | `KEEP RUST` | Storage boundary |
| Cryptography / keyring / OAuth | Yes | Not required by core | Security ecosystem | Rust crates | `KEEP RUST` | Auth boundary |
| Terminal / logging | Yes | Not required by core | TUI/log redaction contracts | Crossterm/Ratatui/tracing | `KEEP RUST` | Observability rules |

## Evidence links

- [Mojo compilation targets and emit options](https://docs.modular.com/mojo/tools/compilation/)
- [`@export` documentation](https://docs.modular.com/mojo/manual/decorators/export)
- [Mojo C ABI and `abi("C")` changelog](https://docs.modular.com/mojo/changelog/v0.26.2/)
- [Mojo FFI standard library](https://docs.modular.com/mojo/std/ffi/)
- [Mojo packages](https://docs.modular.com/mojo/manual/packages/)
- [Official Mojo installation](https://docs.modular.com/mojo/manual/install/)

## Real Mojo CI evidence contract

The `Real Mojo / parity` job in `.github/workflows/ci.yml` compiles the checked-out
`.mojo` sources, links their archives into Rust, runs the Mojo-backed quota and runtime
proxy tests, and runs the built `prodex --version` binary. The lane uses
`PRODEX_MOJO_REQUIRED=1`; `prodex_mojo_active` and the absence of
`prodex_mojo_fallback` are asserted by tests. Rust-only CI keeps the default fallback
behavior and does not satisfy this evidence contract.

## Shared core and release evidence

The current production feature set is `mojo-core`, composed of `mojo-quota`, `mojo-runtime`,
and `mojo-routing`. It compiles `quota.mojo`, `runtime_quota.mojo`, `smart_context.mojo`,
and `routing_score.mojo` into one static archive. Real Mojo CI executes quota, runtime quota,
Smart Context byte estimation, and provider-routing parity through their Rust consumers.

The release matrix is intentionally stricter than object generation:

| Target | Object probe | Final release status | Reason |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Verified with target triple and `x86-64` CPU | `MOJO_RELEASE_SUPPORTED` | Cross-container final link passed with GLIBC_2.18 maximum, no dynamic Mojo dependency/RPATH, and clean execution without `mojo` |
| `aarch64-unknown-linux-gnu` | Verified object only | `RUST_RELEASE_ONLY` | Final archive/link/emulation evidence is not yet available |
| `x86_64-apple-darwin`, `aarch64-apple-darwin` | Verified objects only | `RUST_RELEASE_ONLY` | Signing, release link, and clean-machine evidence are not yet available |
| `*-pc-windows-msvc` | Verified objects only | `RUST_RELEASE_ONLY` | Native MSVC final link/runtime evidence is not yet available |

`PRODEX_MOJO_REQUIRED=1` is mandatory for Real Mojo CI and Mojo-enabled release jobs. The
release job must not downgrade a configured Mojo target to Rust. Installer metadata is rendered
from `migration/release-target-matrix.tsv`; the installer consumes the checksummed TSV and the
binary's own `doctor --runtime --json` implementation/self-test fields, never the user's Mojo
installation state.
