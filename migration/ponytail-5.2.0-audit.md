# Ponytail 5.2.0 source audit

Verified 2026-10-13 for the Prodex 0.437.8 publication freshness gate.
The previous 5.1.1 audit is retained as evidence; upstream published 5.2.0
while the candidate's CI was still running.

- Repository: `https://github.com/DietrichGebert/ponytail`
- Release: `v5.2.0` (stable, published 2026-10-13T01:48:47Z)
- Commit: `2961b52fc1fe6a5a87c7a959a79512e5c2f3ffe5`
- Git tree: `ce7bdb1a8ff7666d8040f34a673598b823fc32ef`
- Source archive SHA-256: `185632166ea37e420e247afc34dd93d56f2156652e5b58d967b820fb6fe7d7fb`
- Prodex tree SHA-256: `9c69590fca3fac59c14d33e498755ba1960590b90837c4e3e1fb4de0e39d3fa6`

All 202 regular files (10583338 bytes) match the Git blob IDs
from the official recursive tree. There are no symlinks, unsupported entries,
or tree-limit exceptions. The Prodex digest uses
`prodex-ponytail-tree-v1\0`, sorted normalized paths, a NUL separator,
eight-byte big-endian lengths, and exact file contents.

Both Codex and Claude manifests identify `ponytail` version `5.2.0`,
retain the existing hook path, and retain the Codex skills directory.
The upstream changes preserve the selected level across context compaction
and flush hook output synchronously. They include the 5.1.1 bounded mapping
and repository-text/symlink isolation fixes.

No hook or installer was executed. Existing managed tool checkouts and
active sessions were not replaced. Minimum support and legacy-manifest
validation rules remain unchanged.
