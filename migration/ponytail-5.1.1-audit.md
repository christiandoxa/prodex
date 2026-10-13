# Ponytail 5.1.1 source audit

Verified 2026-10-13 for the Prodex 0.437.8 publication freshness gate.

- Repository: `https://github.com/DietrichGebert/ponytail`
- Release: `v5.1.1` (stable, published 2026-10-12T23:18:11Z)
- Commit: `6135ace05cd72736594fcf2c6830bea22780f91a`
- Git tree: `279cb66a349c84d6156350792778928a2835ae3d`
- Source archive SHA-256: `8fc49ddace7e8830369358ea80bbbe9a8c1bd8de717fb9a6c5f1015871cd177e`
- Prodex tree SHA-256: `8ff072f3252c87852edc41864511bd9021afe2383998737fa404bdf2d61f39c2`

All 202 regular files (10571841 bytes) in the official source archive
match their Git blob IDs from the recursive tree. There are no symlinks,
unsupported filesystem entries, or exceptions to existing tree limits.
The Prodex digest uses `prodex-ponytail-tree-v1\0`, normalized sorted paths,
a NUL separator, eight-byte big-endian lengths, and exact file contents.

Both Codex and Claude plugin manifests identify `ponytail` version `5.1.1`
and retain `hooks/claude-codex-hooks.json`; the Codex skills path is `skills/`.
The upstream fixes bound optional SessionStart mapping and exclude hostile
repository-controlled text and symlinked files. Node launcher and compaction
compatibility fixes are included by upstream.

No upstream hook or installer was executed during the audit. Existing running
sessions and managed tool checkouts were not replaced. The minimum supported
version and verified legacy-manifest policy remain unchanged.
