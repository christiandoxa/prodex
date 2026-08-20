# Release target matrix

The release workflow consumes `release-target-matrix.tsv`. The installer-facing
manifest is rendered from that file and is published beside `SHA256SUMS`.

| Rust target | Runner | Implementation | Mojo target/CPU | Link and execution evidence | Recommendation |
| --- | --- | --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` + cross | compiled-in Mojo | `x86_64-unknown-linux-gnu` / `x86-64` | Cross-container release link, GLIBC_2.18 maximum, static Mojo dependency audit, clean `doctor --runtime --json` smoke, and Mojo self-test pass locally | `MOJO_RELEASE_SUPPORTED` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` + cross | Rust | `aarch64-unknown-linux-gnu` / `generic` object probe only | Cross Mojo object generation is proven; final release link/execution is not yet enabled | `RUST_RELEASE_ONLY` |
| `x86_64-apple-darwin` | `macos-15-intel` | Rust | `x86_64-apple-darwin` / `x86-64` object probe only | Final Mojo release link and clean-machine smoke remain unproven | `RUST_RELEASE_ONLY` |
| `aarch64-apple-darwin` | `macos-14` | Rust | `aarch64-apple-darwin` / `generic` object probe only | Final Mojo release link, signing, and clean-machine smoke remain unproven | `RUST_RELEASE_ONLY` |
| `x86_64-pc-windows-msvc` | `windows-latest` | Rust | Windows COFF object probe only | Native MSVC final link/runtime and packaging remain unproven | `RUST_RELEASE_ONLY` |
| `aarch64-pc-windows-msvc` | `windows-latest` | Rust | Windows ARM64 object probe only | Native MSVC final link/runtime and packaging remain unproven | `RUST_RELEASE_ONLY` |

The current Mojo compiler (`1.0.0`) accepts target-aware object generation for
the listed triples. Object generation alone does not promote a target. Linux
x86_64 has now passed the cross-container final-link, GLIBC, runtime dependency,
and clean-machine evidence locally; the exact release workflow remains
fail-closed and must repeat those checks for the release SHA.

For Mojo-enabled release rows:

- `PRODEX_MOJO_REQUIRED=1` is mandatory;
- `PRODEX_MOJO_TARGET` and `PRODEX_MOJO_TARGET_CPU` are explicit;
- the final binary must report `mojo-compiled-in` and pass its deterministic
  local self-test;
- the final binary is inspected for dynamic dependencies, RPATH, GLIBC, and
  target CPU assumptions.

End users never install Mojo. Rust-only rows remain supported compatibility
artifacts and may be promoted independently after evidence is complete.
