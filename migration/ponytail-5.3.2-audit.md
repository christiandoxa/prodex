# Ponytail 5.3.2 source audit

Verified for the Prodex 0.438.1 release-cut freshness gate on 2026-10-17.
The historical 5.2.0 audit is retained.

- Repository: `https://github.com/DietrichGebert/ponytail`
- Stable release: `v5.3.2`, published 2026-10-16T23:15:15Z
- Commit: `16352ac229eb08a9598dcae48fe9f2b342567d86`
- Git tree: `e4f281dcbb2eeefa13b330a8161478ddd3057160`
- Source archive SHA-256: `9bbd351988e54c28691750752316acbaffc76778e04b19bc580f067ff77e4747`
- Prodex tree SHA-256: `8609325761024ea38a22f782fa49feae2c068ede9b31b42f03c224fd990f640e`

All 213 regular files (12055164 bytes) in the
commit-addressed source archive match the official recursive Git tree's blob
IDs. The tree has no symlinks, unsupported entries, path traversal, or file/count
budget exceptions. Digest calculation uses the existing
`prodex-ponytail-tree-v1` domain followed by NUL, sorted UTF-8 normalized paths,
NUL separators, eight-byte big-endian lengths, and exact file bytes.

The Codex and Claude manifests both identify Ponytail 5.3.2. Codex retains
`./skills/` and `./hooks/claude-codex-hooks.json`; Claude retains that shared
hook file alongside its additional band hooks. The source license is MIT.
No hook, installer, or historical-session import was executed. Existing managed
checkouts, compatible minimum 4.9.0, and the historical 4.10.0 manifest/digest
rules are unchanged. Full source and checksum evidence was collected before
updating the release reference; this is not a freshness-gate waiver.
