# Codex `rust-v0.152.1` compatibility audit

## Provenance

- Repository: <https://github.com/openai/codex>
- Previous tag object: `7f6bee13af649d0da23ac0c2bf5c83f571fcd611`
- Previous peeled commit: `316795b3cf2a45e90d121d9f46499d4658b2645c`
- Target tag object: `3c6cfbab81e44218c729dc8c6b304cb760d1b8a1`
- Target peeled commit: `5adb68a49933ae446bf11935662c83dba55a0804`
- Target release: `0.152.1`, published `2026-09-01T22:33:02Z`, non-prerelease
- Comparison: `rust-v0.152.0...rust-v0.152.1` (3 commits; 12 files)
- Substantive upstream PR: [#41919](https://github.com/openai/codex/pull/41919), commit `796a15132358f30a24f342e7a8956f77e5d13ca3`

The npm main package and all six platform variants are published at `0.152.1`.
The registry metadata records these exact tarballs and integrity values:

| Package artifact | Tarball | Integrity |
| --- | --- | --- |
| main | `codex-0.152.1.tgz` | `sha512-dSwQzl6JgsFe8L9i8xUnwRz9Vy8gn4UvXFU9xq2IJ1eC7zsSttqQ2SGq49ZZIjEyZQ0LZjCs6Bvtxort2Iyebg==` |
| linux-x64 | `codex-0.152.1-linux-x64.tgz` | `sha512-ar59rr3CX5j4MLMnRcHqcE0eHZPsZlmXlz37ZS2yP3BsV5pNhO+wFXTOzXFdaYmg2cALX7a3Eqv+vB2jQlXnjQ==` |
| linux-arm64 | `codex-0.152.1-linux-arm64.tgz` | `sha512-qZXqf7fxn/SCmaJW6tYrzWqwcDo0gMDJjj1Pm4OtrWXR7Oc0Y2e8ngAh/Mep9iFhVbsqntY1eGLaQaXssGvFgA==` |
| darwin-x64 | `codex-0.152.1-darwin-x64.tgz` | `sha512-M2qW7YkRx+JeSFoZQsrjgA5yNglowuNAFOwRJoIjlgeP8bsyOqPtbSolu3w4Us7IyCH8f/yuKtlt/v/MdDqbfA==` |
| darwin-arm64 | `codex-0.152.1-darwin-arm64.tgz` | `sha512-H8i0uZHILM0Z2Ep+MryCF5rGXmXjmXTzXf5ZK6bobKtZc2yfomi42ZrQWuYQ5P02H0oLG7B5jLaSWZQ+VFgjbA==` |
| win32-x64 | `codex-0.152.1-win32-x64.tgz` | `sha512-B8h0/2Kt+rKQv2+vqBhlhWkMEdhf4dsn46FNKMEBTXj3YC5hwSioOcTX2hMgJxMEMtKIMH6Ire1eNrQPvaL9og==` |
| win32-arm64 | `codex-0.152.1-win32-arm64.tgz` | `sha512-YZjWCcArfSLlqG/4r2Ox5ZZhz1FAFQBZisz8U8r5JLxeLk0tXwZHleu8RjNjly++0S5zsgPtAuF0viSIj7NyRA==` |

## Relevant upstream delta

| Upstream change | Prodex boundary | Decision |
| --- | --- | --- |
| PR #41919 adds `auto_review.node_repl_policy` model metadata. Guardian uses the configured policy, falls back to the bundled policy when absent, skips injection when explicitly empty, and includes the effective policy in reviewer-session reuse and parent-model fallback checks. | Prodex does not own Guardian review semantics or model-policy injection. Its protocol and model metadata boundaries must retain additive upstream fields. | No runtime implementation. Preserve the field opaquely and do not normalize, interpret, or reimplement Guardian behavior. |
| The other two commits prepare the 0.152 release branch and stamp release chores. | No Prodex runtime or transport contract changes. | No code change. |

## Prodex compatibility findings

The comparison does not change Codex Responses, compact, SSE, WebSocket,
app-server, MCP, or auth transport contracts used by Prodex. Existing ping,
app-server, transport, MCP, and authentication boundary tests remain the
regression checks. The active baseline advances to `rust-v0.152.1`; the
historical `migration/codex-rust-v0.152.0-audit.md` and release notes remain
unchanged.

Codex owns Guardian semantics. Prodex preserves opaque/additive metadata and
does not add a competing Guardian implementation.

Source references:

- <https://github.com/openai/codex/compare/rust-v0.152.0...rust-v0.152.1>
- <https://github.com/openai/codex/releases/tag/rust-v0.152.1>
- <https://github.com/openai/codex/blob/rust-v0.152.1/codex-rs/core/src/context/guardian_node_repl_policy.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.1/codex-rs/core/src/guardian/review_session.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.1/codex-rs/protocol/src/openai_models.rs>
