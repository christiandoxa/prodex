# Kiro CLI 2.33.0 release-smoke audit

Verified on 2026-10-21 for the Prodex 0.438.4 standalone publication gate.
The previous 2.32.0 qualification is historical, not the current stable pin.

- Official stable manifest: `https://prod.download.cli.kiro.dev/stable/latest/manifest.json`
- Manifest version: `2.33.0`
- Manifest SHA-256: `0f7daafab61c234ce55cd583733322147a76b05c8d43d29e8714e8721b417b85`
- Version-addressed Linux archive: `https://prod.download.cli.kiro.dev/stable/2.33.0/kirocli-x86_64-linux.tar.gz`
- Archive bytes: `155030330`
- Archive SHA-256: `fdce2898458e23346e63c7a87959cf458290bc44c080951b099c03862056933b`
- Extracted kiro-cli SHA-256: `a2960fad7e3a5c5d36412c9c5148f5e02e49ca90b3202d06f58446e238e1cebd`
- Extracted kiro-cli-chat SHA-256: `acda392c3cac10fc24c72025ada0f9c710034af1eb9b932812938720b3e663bd`

The downloaded Linux x86-64 headless archive matches the official manifest's
size and SHA-256. Before extraction, all ten archive member paths and the
selected binary types and sizes were checked; only the two native CLI binaries
were extracted, without running any installer. Both report 2.33.0, and the chat
help retains --model. The kiro-cli-chat executable retains KIRO_DATA_DIR.
Checks used isolated HOME, XDG and KIRO_DATA_DIR locations with no user
credentials or login. No chat request, provider call, local daemon replacement,
or plugin hook was invoked.

The live Kiro installer still matches SHA-256
`d8dd659c11c156002942f09443b6cb7904959004d4e7f1f73777fdbb2d909b54`.
The separately pinned Copilot and Claude installer hashes were also fetched
and matched without execution. The existing mandatory release checks remain
intact: the stable Kiro manifest must match the qualified pin before binary
build fan-out, and native version/discovery checks must pass after build.

The initial 0.438.4 publication attempt stopped at the stable-manifest check
because the official stable release had advanced from 2.32.0 to 2.33.0.
This qualification permits the canonical retry with the existing 0.438.4
release marker; no new release version or duplicate release marker is needed.
