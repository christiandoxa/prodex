# Mojo ecosystem and runtime evidence

Audit date: 2026-08-26. The pinned compiler is `Mojo 1.0.0 (ed45d567)`. The
experiments below used source checkouts at the recorded commits and did not add
third-party packages to the Prodex release artifact.

## Package decision matrix

| Package/capability | Version/SHA | Compiler compatibility | License | Runtime impact | Prodex use | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| EmberJson | 0.3.4 / `951f4ef28d0c2748a30b2c5e43e139411ccca5ef` | Requires `>=1.0.0b3.dev2026072406`; pinned 1.0.0 failed in reflection at the unresolved `size` declaration | Apache-2.0 | Not measured after compile failure; its documented tape/value path uses owning runtime facilities | No JSON migration; architecture retained as a future experiment | `REFERENCE_ONLY` |
| ExtraMojo | 0.23.0 / `b9d8ee5479f02c007e4d92700e5242928afbd0d8` | The bounded `test_bstr.mojo` source compiled with Mojo 1.0.0 | Unlicense OR MIT | Package recipe requires MAX 26.5.0; no package was linked into Prodex | Byte/string API reviewed; no dependency adopted | `EXPERIMENT` |
| mojo-regex | 0.21.0 / `c4352cbf4f736c2c0e473cb94fff6e476ff82daa` | Source parser test compiled with Mojo 1.0.0 when `src/` was supplied | MIT | Native package/runtime artifact was not approved or linked | Scanner selected for the bounded Prodex grammars; regex remains unnecessary | `REFERENCE_ONLY` |
| ArgMojo | 0.8.0 / `2ba77c1be364e49fe7db88c724dd0f9a25ed3a44` | Declares `>=1.0.0,<1.1.0`; parse test compiled | Apache-2.0 | CLI package is outside the release path | Main Clap CLI remains Rust; no standalone tool needs it | `REFERENCE_ONLY` |
| uuid | 1.1.0 / `c7da63d03cae3b638f1fdaded735c16f619d31f3` (recipe source rev `77613ab60c9fcf73c848c5cdd1df5d1cc432ff3d`) | Pinned source required unavailable `crypto` and used rejected direct UTF-8 indexing under Mojo 1.0.0 | MIT | Crypto dependency and generation path were not considered for Prodex | Rust UUID generation/parsing remains the security-compatible owner | `REJECT` |
| mojo-libc | 0.1.13 / `945fc92c4f462dbefba76c40faee28e315fc6f76` | Declares MAX `>=25.1`; not needed by the deterministic core | MIT | Adds OS/native boundary surface | Rust remains the system host | `EXPERIMENT` |
| Mojo stdlib `String/List/Dict/Set` | compiler-owned | Rich capability probe compiled and ran on 1.0.0 | Mojo distribution terms | Executable linked `libKGENCompilerRTShared.so`, `libMSupportGlobals.so`, `libAsyncRTRuntimeGlobals.so`, a developer RUNPATH, and GLIBC 2.34/2.14 symbols | No owning collection in the release core | `REFERENCE_ONLY` |

The live [Mojo package catalog](https://mojolang.org/packages/) and the
[official packaging guidance](https://mojolang.org/docs/tools/packaging/) were
checked before the source probes. Community package resolution through Pixi or
Conda is a build mechanism only; no mutable package download occurs in
`build.rs`, and no end user needs Pixi/Conda.

## Reproducible probes

The local commands and observed outcomes were:

```text
mojo --version
Mojo 1.0.0 (ed45d567)

mojo build migration/rich_capability_probe.mojo -o /tmp/prodex-rich-capability-20260826
PASS; executable ran

ldd /tmp/prodex-rich-capability-20260826
FAIL for release approval; libKGENCompilerRTShared.so, libMSupportGlobals.so,
libAsyncRTRuntimeGlobals.so, and a developer RUNPATH were present

mojo build EmberJson/test_float_roundtrip.mojo -I EmberJson
FAIL; reflection.mojo used unknown declaration `size`

mojo build ExtraMojo/tests/test_bstr.mojo -I ExtraMojo
PASS

mojo build mojo-regex/tests/test_parser.mojo -I mojo-regex/src
PASS

mojo build ArgMojo/tests/test_parse.mojo -I ArgMojo/src
PASS

mojo build uuid/conda.recipe/test.mojo -I uuid/src
FAIL; `crypto` was unavailable and UUID used unsupported direct String indexing
```

The new release path is `ARENA_MOJO`: `StringSlice`/borrowed views, Mojo
structs, bounded record arrays, open-addressing scratch tables, and caller-owned
byte arenas. The native heap strategy remains `EXPERIMENT`/reference evidence;
the isolated-process strategy is not used for request-time routing because its
latency and lifecycle cost have no architectural benefit for these coarse
deterministic calls.

## Artifact comparison

| Strategy | Measured evidence | Decision |
| --- | --- | --- |
| Existing flat Mojo | Existing release archive remained static and has no approved runtime library | Retain |
| Stdlib owning objects | 49,488-byte capability executable; KGEN/AsyncRT/MSupport libraries, RUNPATH, GLIBC 2.34 observed | Do not promote |
| EmberJson | Compile gate failed before a final artifact; no dependency accepted | Reference only |
| Arena-backed Mojo | Rich domain object probe: 37,840 bytes; context object probe: 17,760 bytes; linked Prodex archive remained KGEN-free, and strict Rust callers passed | Promote |

Object sizes are diagnostic evidence, not a stable release API: link-time
section selection can change them. The final binary, archive, GLIBC, and
clean-machine checks remain authoritative.
