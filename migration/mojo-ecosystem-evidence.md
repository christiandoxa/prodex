# Mojo ecosystem and runtime evidence

Audit date: 2026-08-27. The pinned compiler is `Mojo 1.0.0 (ed45d567)`. The
experiments below used source checkouts at the recorded commits and did not add
third-party packages to the Prodex release artifact.

## Package decision matrix

| Package/capability | Version/SHA | Compiler compatibility | License | Runtime impact | Prodex use | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| EmberJson | 0.3.4 / `951f4ef28d0c2748a30b2c5e43e139411ccca5ef` | Requires `>=1.0.0b3.dev2026072406`; pinned 1.0.0 failed in reflection at the unresolved `size` declaration | MIT | Not measured after compile failure; owning runtime requirements remain unapproved | No JSON migration; bounded Rust external parsing remains the boundary | `REJECT_COMPILER` |
| ExtraMojo | 0.23.0 / `b9d8ee5479f02c007e4d92700e5242928afbd0d8` | The bounded bstr source compiled with Mojo 1.0.0 | Unlicense OR MIT | Linked test requires KGEN/AsyncRT/MSupport libraries and a developer RUNPATH | Stdlib/bounded local scanner retained | `REJECT_RUNTIME` |
| mojo-regex | 0.21.0 / `c4352cbf4f736c2c0e473cb94fff6e476ff82daa` | Source parser test compiled with Mojo 1.0.0 when `src/` was supplied | MIT | Declares Linux/macOS only; no Windows release evidence | Bounded local scanner retained | `REJECT_PLATFORM` |
| ArgMojo | 0.8.0 / `2ba77c1be364e49fe7db88c724dd0f9a25ed3a44` | Declares `>=1.0.0,<1.1.0`; parse test compiled | Apache-2.0 | Main Clap CLI is a Rust system boundary and the package has no Windows target | No relevant Mojo CLI consumer | `NOT_RELEVANT` |
| uuid | 1.1.0 / `c7da63d03cae3b638f1fdaded735c16f619d31f3` | Direct String indexing is rejected by Mojo 1.0.0 | MIT | Declares no Windows target and has a compiler-incompatible implementation | Rust UUID security boundary retained; no Mojo generation | `REJECT_COMPILER` |
| mojo-libc | 0.1.13 / `945fc92c4f462dbefba76c40faee28e315fc6f76` | Declares MAX `>=25.1`; not needed by the deterministic core | MIT | Adds OS/native boundary surface | Rust remains the system host | `NOT_RELEVANT` |
| decimo | 0.14.0 / `6e638280762a611be8882ec45bef47a26fe9e1ef` | One bounded test compiled with Mojo 1.0.0 | Apache-2.0 | No Mojo-owned arbitrary-precision accounting path exists | Durable accounting remains Rust | `NOT_RELEVANT` |
| Mojo stdlib `String/List/Dict/Set` | compiler-owned | Rich capability probe compiled and ran on 1.0.0 | Mojo distribution terms | Owning collections link KGEN/AsyncRT/MSupport and raise the deployment baseline | Borrowed views and caller-owned bounded tables retained | `ADOPT_STDLIB` for release-safe capabilities |

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
FAIL; UUID used unsupported direct String indexing under Mojo 1.0.0

mojo build decimo/tests/test_decimal.mojo -I decimo/src
PASS; no Prodex decimal consumer justified adoption
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
