# Codex 0.153.4 queued-input TUI compatibility

Prodex uses the official Codex `rust-v0.153.4` source at commit
`3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`. That release emits the supported
`thread/queue/changed` notification, but its TUI does not render the
authoritative queue state.

The adjacent patch applies only to the Codex TUI. It refreshes
`thread/queue/list` for the exact active thread and renders bounded queued
text through the existing pending-input preview. The server queue remains the
only source of truth; consumed entries are removed by the next queue snapshot.

Patch SHA-256:

`6c2dd2dae167c687bc2870082815a62f1c191e34f5b323f98ea442abfd11859b`

The patch is reproducible from the pinned upstream commit and contains no
transport, terminal, transcript, or persistence workaround.

Linux release builds also apply
`codex-rust-v0.153.4-linux-openssl.patch`. It enables the lockfile-pinned
vendored OpenSSL source for portable GNU/Linux binaries without a system
OpenSSL runtime dependency.

Linux OpenSSL patch SHA-256:

`f1958eb45138a5c3ca0a03d0976f22c6d59ab1fc18f2fbda9b68a59c3ec40e12`
