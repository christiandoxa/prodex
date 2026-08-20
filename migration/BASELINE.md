# Migration baseline

Captured for the current descendant migration pass on 2026-08-20.

| Item | Value |
| --- | --- |
| Effective Prodex commit | `99f73371784c1ef4f38dd44060eadca0b126a78a` (`feat(mojo): expand quota core and enforce real CI parity`) |
| Prodex version | `0.410.0` |
| Rust toolchain | `rustc 1.97.1 (8bab26f4f 2026-07-14)`; `cargo 1.97.1 (c980f4866 2026-06-30)` |
| Cargo.lock SHA-256 | Updated by this uncommitted migration; recompute at commit time (`b0cd59526ef4a886b550f7b81e0b45791eba7b3840343170c7ebfde186fd9e2a`) |
| Mojo compiler | `Mojo 1.0.0 (ed45d567)` |
| Modular CLI | No `modular` executable installed locally |
| Platform | Linux `7.0.0-29-generic` |
| Architecture | `x86_64` |
| Existing bridge | Shared `prodex-mojo-core`, disabled by default; Rust fallback retained |
| Strict Mojo mode | `PRODEX_MOJO_REQUIRED=1`; missing compiler/archiver or disabled Mojo feature is a hard failure |
| Real Mojo CI | `Real Mojo / parity` in `.github/workflows/ci.yml`, Ubuntu 24.04, `mojo==1.0.0` via official uv install |

The baseline worktree was clean and matched `origin/main` before this pass. Existing
Rust-only and Mojo-enabled bridge tests are the behavioral starting point.
