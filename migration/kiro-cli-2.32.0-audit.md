# Kiro CLI 2.32.0 release-smoke audit

Verified on 2026-10-17 for the Prodex 0.438.1 standalone publication gate.
The previous 2.31.1 qualification is historical, not the current stable pin.

- Official stable manifest: `https://prod.download.cli.kiro.dev/stable/latest/manifest.json`
- Manifest version: `2.32.0`
- Manifest SHA-256: `1997883a34c863cee7dc92eaa8cf340f1b50de84fbf4bfdc5434ddc99aaeaa45`
- Version-addressed Linux archive: `https://prod.download.cli.kiro.dev/stable/2.32.0/kirocli-x86_64-linux.tar.gz`
- Archive bytes: `154977733`
- Archive SHA-256: `05989f9bfe465b304e8d1b2dba50aec2b84fcc1deaa4c0773af3f539d6e0d312`
- Extracted kiro-cli SHA-256: `58abd8b474b31588c463d321d70dc223a1ea4bbed9048c163a8d485f0f05e4dd`
- Extracted kiro-cli-chat SHA-256: `5ba2a13daaed1146533b6274d5967a4b0a20a5ffe85e00f5fdb1a2219d2d48e9`

The downloaded Linux x86-64 headless archive matches the official manifest's
size and SHA-256. Before extraction, archive member paths and extracted binary
sizes were checked; only the two native CLI binaries were extracted, without
running any installer. Both report 2.32.0, and the chat help retains --model.
The kiro-cli-chat executable retains KIRO_DATA_DIR. Checks used isolated
HOME, XDG and KIRO_DATA_DIR locations with no user credentials or login.
No chat request, provider call, local daemon replacement, or plugin hook ran.

The live Kiro installer still matches SHA-256
`d8dd659c11c156002942f09443b6cb7904959004d4e7f1f73777fdbb2d909b54`.
The separately pinned Copilot and Claude installer hashes were also fetched
and matched without execution. The existing mandatory release checks remain
intact: the stable Kiro manifest must match the qualified pin before binary
build fan-out, and native version/discovery checks must pass after build.
