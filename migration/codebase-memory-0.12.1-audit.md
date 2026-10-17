# Codebase Memory MCP 0.12.1 release audit

Verified for Prodex 0.438.1 on 2026-10-17 after the optional-tool freshness gate
reported a newer stable upstream release.

- Repository: `https://github.com/DeusData/codebase-memory-mcp`
- Stable release: `v0.12.1`, published 2026-10-16T23:35:48Z
- Commit: `a7158ac2f48ca646b4527341b76f930f2aeff87e`
- Git tree: `2297ca5b45b72e0f80805294c34d3d87957d45ec`
- Checksum manifest SHA-256: `103ddf75ccbe4e4de9faa57f074ee5f16b88edab6fd2f7dfd34e50d147dddffe`
- Portable Linux executable SHA-256: `77745491f0a9c346d6da23eed4d92b5feaa31a27a883c9faa4fa9f47c8183705`

All six supported official platform archives were downloaded. Each container
hash matches both the upstream `checksums.txt` and the GitHub release asset
digest. Linux/macOS containers are tar.gz; Windows containers are zip.

| Platform | Downloaded archive SHA-256 |
| --- | --- |
| `linux-amd64-portable` | `8c3d0341064301e78ecc84cf7348d4e2c6562a0f57448243ea560aaa34de36c1` |
| `linux-arm64-portable` | `ed0f1f548cd79b121c401c2e9f7841c189a001af26d3768368088c24f6a53533` |
| `darwin-amd64` | `ca6e64397a8ae80cec898a8205a6aa16750ac798acc05a0c3ed983837944a581` |
| `darwin-arm64` | `9bfcbe38414b47969a3e1d1c1999f5357409387c86fa3d24221b5458267660d2` |
| `windows-amd64` | `4e4e0f16b45233af5c757a1fcbe3c3b16e9056375b1fbe2ce020b6de75dedcc0` |
| `windows-arm64` | `934ab6bd67a76c6ebd9e110d87b7090ffd629919bedaeddabe5a19175038b063` |

The Linux portable archive contains one executable, its license and notices,
and an installer. Only the verified executable was extracted; archive members
were checked for path traversal, links, unexpected types, and bounded sizes.
The installer was not executed. With isolated HOME/XDG/cache variables the
binary reported `codebase-memory-mcp 0.12.1`. Its read-only `daemon status`
command returned the accepted active-daemon contract and observed the already
running per-account 0.12.0 daemon; HOME isolation does not isolate that daemon.
No daemon start, stop, replacement, or user indexing command was issued.

The minimum remains 0.9.1-rc.1. Upstream documents that a user-initiated upgrade
rebuilds each existing project on its next indexing run; qualification does not
trigger that rebuild. Optional installation remains under user control.
