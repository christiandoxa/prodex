# Release target matrix

The release workflow consumes `release-target-matrix.tsv`. The installer-facing
manifest is rendered from that file and is published beside `SHA256SUMS`.

| Rust target | Runner | Implementation | Mojo target/CPU | Link and execution evidence | Recommendation |
| --- | --- | --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` + cross | compiled-in Mojo | `x86_64-unknown-linux-gnu` / `x86-64` | Cross-container release link, GLIBC_2.23 maximum, static Mojo dependency audit, clean `doctor --runtime --json` smoke, and Mojo self-test pass locally | `MOJO_RELEASE_SUPPORTED` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` + cross | compiled-in Mojo | `aarch64-unknown-linux-gnu` / `generic` | Cross-container release link, GLIBC_2.23 maximum, static Mojo dependency audit, clean emulated `doctor --runtime --json` smoke, and Mojo self-test pass locally | `MOJO_RELEASE_SUPPORTED` |
| `x86_64-apple-darwin` | `macos-15-intel` | Mojo archive + native Rust link | Linux-hosted `x86_64-apple-darwin` / `x86-64` archive, native macOS link and smoke | Release workflow fails closed if cross archive or native Mojo doctor evidence is missing | `MOJO_RELEASE_SUPPORTED` |
| `aarch64-apple-darwin` | `macos-14` | Mojo archive + native Rust link | Linux-hosted `aarch64-apple-darwin` / `generic` archive, native macOS link and smoke | Release workflow fails closed if cross archive or native Mojo doctor evidence is missing | `MOJO_RELEASE_SUPPORTED` |
| `x86_64-pc-windows-msvc` | `windows-latest` | Mojo archive + native Rust link | Linux-hosted Windows COFF `x86-64` archive, native MSVC link and smoke | Release workflow fails closed if cross archive or native Mojo doctor evidence is missing | `MOJO_RELEASE_SUPPORTED` |
| `aarch64-pc-windows-msvc` | `windows-latest` | Mojo archive + native Rust link | Linux-hosted Windows ARM64 `generic` archive, native MSVC link, and strict archive/dependency verification; the hosted x64 runner cannot execute the ARM64 PE | Release workflow fails closed if the cross archive, native link, or static Mojo evidence is missing | `MOJO_RELEASE_SUPPORTED` |

The current Mojo compiler (`1.0.0`) accepts target-aware object generation for
the listed triples. Object generation alone does not promote a target. Linux
x86_64 and aarch64 Linux have passed the cross-container final-link, GLIBC,
runtime dependency, and clean-machine evidence locally. The aarch64 lane runs
the final binary through QEMU user emulation. The exact release workflow remains
fail-closed and must repeat those checks for the release SHA.

The promoted UTF-8 context component uses borrowed `StringSlice`, nullable pointer views, and
`InlineArray`; it does not use heap-owning Mojo collections. Release CI rejects
`KGEN_CompilerRT_*` archive references and still requires no Mojo/Modular dynamic dependency or
RPATH. `String`, `List`, `Dict`, and `Set` remain outside release artifacts until a bundled-runtime
design can preserve the target GLIBC and clean-machine contracts.

Rich ABI v2 is included in the same static archive for every release target. It exports the
context diagnostic, route-plan, policy-alias, model-fallback, context-plan, and log-semantics
operations and uses only typed structs, borrowed views, offset slices, caller-owned output, and
bounded scratch tables. The pinned Linux build host cross-compiles target archives; native
macOS/Windows runners perform the final link and clean-runtime evidence.

For Mojo-enabled release rows:

- `PRODEX_MOJO_REQUIRED=1` is mandatory;
- `PRODEX_MOJO_TARGET` and `PRODEX_MOJO_TARGET_CPU` are explicit;
- the final binary must report `mojo-compiled-in` and pass its deterministic
  local self-test;
- the final binary is inspected for dynamic dependencies, RPATH, GLIBC, and
  target CPU assumptions.

End users never install Mojo. A target cannot be published as a Rust-only compatibility artifact
when its row is marked `MOJO_RELEASE_SUPPORTED`; missing Mojo artifacts or failed self-tests
abort that release.
