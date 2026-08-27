# Mojo capability matrix

## Toolchain evidence

| Item | Result |
| --- | --- |
| Local compiler | `Mojo 1.0.0 (ed45d567)` from `mojo --version` |
| Modular CLI | Not installed as a `modular` executable; no command was invented |
| Verified source syntax | `def`, `Int64`, `UInt64`, `Pointer`, `Optional[Pointer]`, `StringSlice`, `Span`, `InlineArray`, structs, reflection layout probes, `@export("...")`, and `abi("C")` |
| Verified artifact | Strict Cargo builds compile numeric and rich text sources into the shared static archive; the archive has no `KGEN_CompilerRT_*` reference |
| Verified Rust call | Strict context/core tests pass UTF-8 string-view records through Mojo text validation, `StringSlice` comparison, duplicate grouping, structured output, and concurrent calls |
| Shared library | `--emit shared-lib` compiled in the spike; not selected for Cargo because it adds runtime loader/distribution state |
| Portability | Target-aware object probes exist for Linux x86_64/aarch64, macOS x86_64/arm64, and Windows COFF; Linux x86_64 and aarch64 have release link/runtime evidence |
| Real Mojo CI | Ubuntu 24.04 installs official `mojo==1.0.0` with pinned uv `0.11.7`; `PRODEX_MOJO_REQUIRED=1` forbids fallback |

Reproducible local probes:

```bash
mojo build migration/rich_capability_probe.mojo -o target/rich_capability_probe
target/rich_capability_probe
mojo build migration/system_capability_probe.mojo -o target/system_capability_probe
target/system_capability_probe
```

The system probe is Linux-specific (`/dev/null` and `printf`) and generates evidence only; it is
not a release utility or a reason to move process/filesystem orchestration from Rust.

`--emit object` is currently documented as experimental. The production text object remains
allocation-free and has no observed Mojo runtime dependency. A compiled and executed local probe
proved that owning `String`, `List`, `Dict`, and `Set` work, but their object references
`libKGENCompilerRTShared.so`; that runtime and its support libraries raise the observed Linux
GLIBC requirement as high as 2.35. Prodex therefore uses zero-copy `StringSlice` plus
`InlineArray` in release code and does not bundle that runtime.

| Capability | Needed by Prodex? | Local status | API maturity for this migration | Rust equivalent | Recommendation | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| Integers / branches | Yes | Verified | Sufficient for bounded scalar and batch cores | Rust integer types | `MOVE NOW` | quota, runtime, and `mojo/prodex_core/routing_score.mojo` |
| Floats | Yes, quota fractions/context scores | Verified for Gemini quota arithmetic | Proven for the existing `f64` ABI and rounding fixtures; keep new float seams separate | Rust `f32`/`f64` | `MOVE NOW` only with exact parity | Gemini batch parity and float probes; official Mojo manual/API |
| `String` | Yes | Compile/link/runtime verified; heap-capable object requires Mojo compiler runtime | Usable internally, not release-safe under current GLIBC/runtime contract | Rust `String` | `EXPERIMENT` | Local `String`/collection executable probe and dependency inspection |
| UTF-8 `StringSlice` | Yes | Production verified without heap allocation | Sufficient for borrowed synchronous text | Rust `str` | `MOJO` | `context_text.mojo`, raw UTF-8 tests, 512-case differential suite |
| `CStringSlice` | Only real C-string APIs | Compile/link/runtime verified | Requires nul termination and rejects interior nul | Rust `CStr` | `KEEP C-ONLY` | `rich_capability_probe.mojo`; not used by general Prodex text ABI |
| Byte view | Yes | Pointer + explicit native length layout verified | Sufficient; text view validates before interpretation | Rust `&[u8]` | `MOVE NOW` | Rust/Mojo size, alignment, offset, embedded-nul, and no-sentinel tests |
| `InlineArray` | Yes | Production verified | Sufficient for fixed seven-field records | Rust arrays | `MOJO` | Critical-signal text row construction |
| `List` / `Dict` / `Set` | Yes | Compile/link/runtime verified with native collections | Requires bundled compiler runtime; not release-promoted | `Vec` / maps / sets | `EXPERIMENT` | Local collection probe; `ldd`, `nm`, and GLIBC inspection |
| Optionality | Yes | Scalar pairs and `Optional[Pointer]` null niche verified | FFI layout pinned by text ABI v1 probes | `Option<T>` | `MOJO` for verified forms | Empty/null and non-null view tests |
| `Variant` | Maybe | Compile/link/runtime verified | Internal use only; no ABI layout contract | Rust enums | `EXPERIMENT` | Local rich runtime probe |
| Error handling | Yes | Mojo raising/catching verified; production uses allocation-free status tags | Structured semantic failures supported | `Result<T,E>` | `MOJO` for tagged ABI results | Malformed UTF-8, capacity, and ABI-version tests |
| Structs / traits | Yes | Internal structs/traits and fixed C-layout records verified | Struct ABI requires explicit layout/version probes | Rust structs/traits | `MOJO` for verified records | `ProdexStringView`, `ContextTextRowsResult`, reflection probe |
| Filesystem / environment | Yes | Local compile/link/runtime probe passed | Available, but Rust remains simpler and established | `std::fs`, `std::env` | `KEEP RUST` | `/dev/null` read and `getenv` probe |
| JSON / serialization | Yes | Not required by core | Serde is compatibility oracle | Serde / `serde_json` | `KEEP RUST` | Existing provider/config code |
| Hashing / cryptography | Yes | Not required by core | Security-sensitive | Rust crypto crates | `KEEP RUST` | Threat model |
| Time / clocks | Yes | Not required by core | Runtime/timezone behavior matters | `chrono`, `std::time` | `KEEP RUST` | Quota reset paths |
| Randomness | Yes | Not required by core | Security/runtime semantics | `getrandom` | `KEEP RUST` | Existing dependency |
| Threads | Yes | Text entry point passed eight-thread reentrancy test | Core stays stateless; Tokio owns orchestration | Tokio/std threads | `KEEP RUST` host | Concurrent Rust-to-Mojo test |
| Async | Yes | Minimal `create_task(...).wait()` runtime probe passed | Broader async compiler defects remain open; no host migration evidence | Tokio | `EXPERIMENT` only | Local probe plus upstream Mojo async defect review |
| Networking / HTTP | Yes | Not required by core | Mature Rust stack exists | Hyper/Reqwest | `KEEP RUST` | Architecture |
| TLS | Yes | Not required by core | Security-sensitive ecosystem | rustls | `KEEP RUST` | Threat model |
| Process execution / PTY | Yes | `std.subprocess.run` compile/link/runtime probe passed; PTY unprobed | Packaging and lifecycle evidence absent | Rust process/PTY crates | `KEEP RUST` | Synthetic `printf` subprocess probe |
| Object/shared library / C ABI | Yes | Static object, shared library, scalar, flat-buffer, string-view, and structured-record exports verified | Static object remains selected; heap-owning types require dynamic runtime | Rust `extern "C"` | `MOVE NOW` for verified kernels | `ffi-contract.md`; object/shared probes; release dependency audit |
| SQLite / PostgreSQL / Redis | Yes | Not required by core | Rust drivers and persistence contracts | rusqlite/postgres/redis | `KEEP RUST` | Storage boundary |
| Cryptography / keyring / OAuth | Yes | Not required by core | Security ecosystem | Rust crates | `KEEP RUST` | Auth boundary |
| Terminal / logging | Yes | Event classification is production-verified; file watching, TUI, and redaction remain Rust | TUI/log redaction contracts | Crossterm/Ratatui/tracing | `MOJO` for bounded event classification; `KEEP RUST` for IO/rendering | `prodex_mojo_log_classify_v3` feeds the shared stream renderer |

## Evidence links

- [Mojo compilation targets and emit options](https://docs.modular.com/mojo/tools/compilation/)
- [`@export` documentation](https://docs.modular.com/mojo/manual/decorators/export)
- [Mojo C ABI and `abi("C")` changelog](https://docs.modular.com/mojo/changelog/v0.26.2/)
- [Mojo FFI standard library](https://docs.modular.com/mojo/std/ffi/)
- [`StringSlice` standard library](https://mojolang.org/docs/std/collections/string/string_slice/StringSlice/)
- [`InlineArray` standard library](https://mojolang.org/docs/std/collections/inline_array/InlineArray/)
- [Mojo packages](https://docs.modular.com/mojo/manual/packages/)
- [Official Mojo installation](https://docs.modular.com/mojo/manual/install/)

## Real Mojo CI evidence contract

The `Real Mojo / parity` job in `.github/workflows/ci.yml` compiles the checked-out
`.mojo` sources, links their archives into Rust, runs the Mojo-backed UTF-8 context pipeline,
quota, Smart Context pressure, runtime candidate ordering, provider-routing, and
capability-negotiation tests, and runs the built `prodex --version` binary. The lane uses
`PRODEX_MOJO_REQUIRED=1`; `prodex_mojo_active` is asserted by tests. Rust-only CI omits
the Mojo feature and does not satisfy this evidence contract.

## Shared core and release evidence

The current production feature set is `mojo-core`, composed of `mojo-quota`, `mojo-runtime`,
and `mojo-routing`. It compiles the quota, runtime, Smart Context, routing, and bounded log
classification sources into one
static archive. Real Mojo CI executes quota, runtime quota, Smart Context byte estimation and
pressure snapshot, runtime candidate ordering, profile scheduling order, context signal
arithmetic, provider score/routing-plan batching, and provider capability matching through their
Rust consumers. The current production feature set also includes provider constraints, runtime
policy numeric validation, and runtime tuning defaults; these reuse shared-core wiring, with one
explicit `mojo-provider-constraints` feature for the provider-core owner.

## 2026-08-21 promotion evidence

| Kernel | Boundary shape | Maximum / tags | Float or strings | Rust retained outside `MOJO` |
| --- | --- | --- | --- | --- |
| Optimistic candidate decision | Fixed scalar integers and normalized booleans | 15 result tags; route/source/quota tags | Rust owns strings and comparisons | Test oracle; Rust-only path |
| Provider request constraints | Versioned 17/7-word input and 12/5-word output flat buffers | Exact counts plus explicit policy/decision/feature/field tags; one evaluation | Rust owns JSON, catalog, provider adapters, and errors | Test oracle; Rust-only path |
| Smart Context rehydration | Parallel cost/required/availability arrays | 256 rows; four action tags | Rust owns artifact IDs, store lookup, and ordering | Test oracle; Rust-only path |
| Quota main aggregation | Parallel presence/value arrays | 1,024 rows; presence flags | Rust owns decimal/floating conversion and reset acquisition | Test oracle; Rust-only path |
| Runtime profile scheduling order | Parallel 16-field `Int64` rows and output indices | 256 rows; stable permutation | Rust owns clock/state/name collection; Mojo derives scaling, reserve bias, and hysteresis ordering | Test oracle; Rust-only path |
| Runtime policy numeric validation | Parallel `UInt64` inputs and caller-owned `Int64` failure flags | Section-sized batches; `NonZero=0`, `Range=1`, `LessOrEqual=2` | Rust owns config parsing, paths, security, and exact errors | Shared primitive evaluator for Rust-only targets |
| Critical-signal loss/gain | Seven `Int64` counters per side and two output arrays | Seven fixed counters | Rust owns line classification; rich text grouping is the next row | Test oracle; Rust-only path |
| Critical-signal text grouping | `ProdexStringView` records, seven counters per line, caller-owned row/hash buffers, structured result | 65,536 lines/keys; text ABI v1 | Mojo validates UTF-8, compares `StringSlice`, groups duplicates, and constructs rows; Rust retains terminal normalization and classification | Exact Rust oracle; Rust-only path |
| Runtime tuning defaults | Seven caller-owned scalar outputs | Bounded integer clamps | Rust owns host/config parsing and overrides | Test oracle; Rust-only path |

All new calls are stateless and reentrant. No Rust object layout, string, collection, secret,
path, persistent state, or provider payload crosses the ABI. The shared ABI version remains `1`
because these are additive exports.

The release matrix is intentionally stricter than object generation:

| Target | Object probe | Final release status | Reason |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Verified with target triple and `x86-64` CPU | `MOJO_RELEASE_SUPPORTED` | Cross-container final link passed with the GLIBC_2.23 release ceiling, no dynamic Mojo dependency/RPATH, and clean execution without `mojo` |
| `aarch64-unknown-linux-gnu` | Verified object and archive | `MOJO_RELEASE_SUPPORTED` | Final cross link, GLIBC_2.23 ceiling, dependency audit, QEMU execution, clean runtime, and Mojo self-test pass |
| `x86_64-apple-darwin`, `aarch64-apple-darwin` | Cross-target objects plus native release link/runtime gate | `MOJO_RELEASE_SUPPORTED` | The release workflow builds the archive on Linux and requires native macOS doctor/self-test evidence |
| `*-pc-windows-msvc` | Cross-target COFF objects plus native release link/runtime gate | `MOJO_RELEASE_SUPPORTED` | The release workflow builds the archive on Linux and requires native MSVC doctor/self-test evidence |

`PRODEX_MOJO_REQUIRED=1` is mandatory for Real Mojo CI and Mojo-enabled release jobs. The
release job must not downgrade a configured Mojo target to Rust. Installer metadata is rendered
from `migration/release-target-matrix.tsv`; the installer consumes the checksummed TSV and the
binary's own `doctor --runtime --json` implementation/self-test fields, never the user's Mojo
installation state.

## Build-time versus runtime Mojo requirement

`compiler_required=false` is intentional for every compiled-in artifact: the shipped binary
does not need a Mojo compiler at runtime. `build_strict=true` records that its build used
the strict Mojo feature path; a missing compiler, archiver, or required target archive fails the
build. These fields must not be treated as synonyms.

## Rich capability promotion (2026-08-26)

This is the final matrix for the rich wave against baseline
`a66c4a54e0eb56229188f63616e05d0698085cb8`.

| Capability | Baseline | Final | Production? | Artifact impact |
| --- | --- | --- | --- | --- |
| StringSlice | MOJO | MOJO | Yes, borrowed UTF-8 context/domain views | Static; no new runtime library |
| String | EXPERIMENT | EXPERIMENT | No; probe only | Probe links KGEN/AsyncRT/MSupport and a developer RUNPATH |
| List | EXPERIMENT | EXPERIMENT | No; Rust adapter storage only | Same owning-runtime risk; not in release archive |
| Dict | EXPERIMENT | EXPERIMENT | No; open-addressing tables used instead | No package/runtime impact |
| Set | EXPERIMENT | EXPERIMENT | No; open-addressing tables used instead | No package/runtime impact |
| Optional | verified forms | verified borrowed-pointer forms | Yes for optional ABI references and domain decisions | Pointer-sized ABI field; no owning allocation |
| Variant | EXPERIMENT | EXPERIMENT | No; explicit bounded tags at ABI edge | Not linked into release core |
| Domain structs | partial | PROMOTED | Yes: `DiagnosticRecord`, `RouteCandidate`, `PolicyRule`, `ContextItem`, `ContextPlan` | Plain Mojo record layout, statically linked |
| Arena objects | none/partial | PROMOTED | Yes: offset slices, record arrays, object indices, scratch hash tables | Caller-owned buffers; no Mojo heap runtime |
| Parser | Rust | MOJO | Yes: `combo:` fallback grammar and route-alias semantic grammar | New rich v4 exports only |
| Structured errors | partial | MOJO | Yes: domain/kind/field/index/offset/length issue records | Fixed-width result fields |
| Structured output strings | limited | MOJO | Yes: normalized keys, provider/model names, model chains, artifact IDs | Rust-owned output byte arenas |
| JSON | Rust | KEEP_RUST | No; Serde remains external compatibility parser | No EmberJson dependency |

Promotion evidence is executable: strict Rust callers run all five rich v4 operations plus the
log-classification export, layout checks
compare compiler-generated sizes/alignment, and the release archive checks all v3 symbols and
rejects `KGEN_CompilerRT_*` references. Native heap and package-backed paths remain non-production
until their full target and clean-machine gates pass.
