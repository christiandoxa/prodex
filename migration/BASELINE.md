# Migration baseline

Captured at the start of the large migration pass on 2026-08-20.

| Item | Value |
| --- | --- |
| Effective Prodex commit | `89869552fa5441be0605f071c4d5ccc1e1693630` (`feat(mojo): add opt-in quota core bridge`) |
| Prodex version | `0.410.0` |
| Rust toolchain | `rustc 1.97.1 (8bab26f4f 2026-07-14)`; `cargo 1.97.1 (c980f4866 2026-06-30)` |
| Cargo.lock SHA-256 | `20f591119cafe2d7a6a8e9257655aa268c231eda5c9fffa0e7bbe67f71904f52` |
| Mojo compiler | `Mojo 1.0.0 (ed45d567)` |
| Modular CLI | No `modular` executable installed locally |
| Platform | Linux `7.0.0-29-generic` |
| Architecture | `x86_64` |
| Existing bridge | `prodex-quota/mojo`, disabled by default; Rust fallback retained |
| Strict Mojo mode | `PRODEX_MOJO_REQUIRED=1`; missing compiler/archiver or disabled Mojo feature is a hard failure |
| Real Mojo CI | `.github/workflows/mojo-parity.yml`, Ubuntu 24.04, `mojo==1.0.0` via official uv install |

The baseline worktree was clean and matched `origin/main` before this pass. Existing
Rust-only and Mojo-enabled bridge tests are the behavioral starting point.
