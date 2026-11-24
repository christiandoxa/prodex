# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.172.1 - 2026-11-24

### Deps

- Refresh stable Rust dependency graph (`3076d5a`)

### Misc

- Align packaging with current CLI targets (`3e3c1cc`)
- Build strict Mojo images on stable dependencies (`da32122`)
# Prodex 0.172.1

Refresh stable dependencies, build tools, native-client smoke inputs, and
container images while retaining the Codex 0.172.0 compatibility baseline.

## New Features

### Dependency and Toolchain Updates

- Integrate Dependabot PR #136 for `astral-sh/setup-uv` 10.4.0.
- Update the maintained Rust dependency declarations and both lockfiles to the
  newest stable versions allowed by the supported dependency graph, including
  Hyper 1.13.0, keyring-core 1.1.0, Redis 1.8.3, and Jiff 1.0.2.
- Pin Node.js 26.13.1, npm 12.2.3, uv 0.13.9, and sccache 0.20.0. Retain the
  current stable Rust 1.100.0, Python 3.15.2, and Mojo 1.2.0 toolchains.
- Refresh the pinned security tools and image digests, including cargo-audit
  0.23.0, cargo-deny 0.22.0, cargo-machete 0.10.1, Syft 1.57.2, and KICS 2.2.1.
- Update native-client release smoke inputs to Gemini CLI 0.71.1, GitHub
  Copilot CLI 1.0.114, and Claude Code 2.1.331.

Upstream dependency constraints remain authoritative. Transitive dependencies
that cannot resolve to a newer major version retain the versions required by
their current stable parent crates. No external npm or Python application
libraries are added.

## Bug Fixes

- Give compiler-cache initialization a bounded startup window before CI compiler
  consumers run, and keep Cargo-dependent container checks in the Rust test lane.

### Container Builds

- Move the Rust builder and runtime bases to Debian 13 (trixie), with exact
  image digests and runtime package versions.
- Include the vendored Cargo patch and Mojo sources in the Docker build
  context, install the pinned Mojo compiler, and require compiled Mojo kernels.
- Package the current `prodex` executable and show CLI help by default. Remove
  Docker references to the retired enterprise gateway and control-plane binaries.
- Verify that the projected Docker build context resolves its local Cargo
  patch; a context missing the vendor directory must fail.

## Changelog

### Database Image Upgrades

The retained legacy Compose template pins PostgreSQL 19.0 and Redis 8.12.0.
It documents the retired enterprise stack and is not a supported deployment
for the current Prodex CLI image. See the [container and legacy database guidance](https://github.com/christiandoxa/prodex/blob/0.172.1/docs/deployment.md).

The PostgreSQL image check verifies that its versioned mount rejects an old
PostgreSQL 16 volume before initialization and can initialize a fresh database.
These image checks do not restore the removed Prodex storage adapters. Preserve
verified backups and original volumes when migrating an independently managed
database.

### Upstream Compatibility

The standalone wrapper continues to use a separately installed
[Codex CLI 0.172.0](https://github.com/openai/codex/releases/tag/rust-v0.172.0).
The GPT-6.2 Sol/Astra catalog, proxy routing, native session resume forms, and
Ornith goal-context adaptation retain their existing contracts.

Full Changelog: [0.172.0...0.172.1](https://github.com/christiandoxa/prodex/compare/0.172.0...0.172.1)

## 0.172.0 - 2026-11-24

### Misc

- Qualify native 0.172 compatibility (`88f4cbf`)

## 0.171.3 - 2026-11-21

### Misc

- Support Codex goals on Ornith Responses (`fddb036`)

## 0.171.2 - 2026-11-21

### Misc

- Reuse Ornith compaction prefix (`77f207f`)

## 0.171.1 - 2026-11-20

### Misc

- Tolerate long-context inference prefill (`fc559d5`)

## 0.171.0 - 2026-11-20

### Claude

- Qualify RTK 1.2.1 and Playwright MCP 0.0.86 (`881bfba`)

### Misc

- Qualify Codex 0.171.0 and pass through remote exec (`30cd378`)

## 0.170.0 - 2026-11-18

### Runtime

- Preserve native HTTP fallback for oversized WebSocket requests (`9defc62`)

### Misc

- Qualify Codex 0.170.0 source and lifecycle contracts (`c887792`)

## 0.169.2 - 2026-11-18

### Deps

- Replace unmaintained Chrono with a Jiff host time boundary (`cc1a6fa`)

### Misc

- Preserve persisted calendar display and timestamp contracts (`44c3391`)
- Qualify Codex 0.169.1 provider and model contracts (`e9b9fa2`)
- Add GPT-6.2 Astra and Sol without overriding native defaults (`f7437e7`)
- Repair external home rollout indexes before launch (`f60da6b`)

## 0.169.1 - 2026-11-16

### Runtime

- Isolate reusable brokers by outbound transport identity (`690d7eb`)
- Honor native proxy transports and custom certificate trust (`fcd85ad`)

### Docs

- Scope SOCKS qualification to owned transport (`c31b316`)
- Distinguish native account discovery from inference transport (`8d2d561`)
- Describe proxy resume and untrusted goal compatibility (`0a6dccc`)

### Deps

- Integrate reviewed Dependabot update #132 (`39a10cd`)
- Integrate reviewed Dependabot update #131 (`800b5f0`)
- Integrate reviewed Dependabot update #130 (`a5ca658`)
- Integrate reviewed Dependabot update #129 (`c0b1558`)
- Bump rust from `9181b56` to `e6f2cff` (`cef1cf5`)
- Bump uuid in /fuzz in the fuzz-cargo group (`45b9943`)
- Bump the cargo group with 5 updates (`2a10970`)

### Misc

- Qualify 0.0.21 and isolate ambient endpoint settings (`5d8797a`)
- Keep external overlay goals and session indexes persistent (`4ad5fc2`)
- Preserve untrusted goal context without replaying stale history (`dc938e4`)

## 0.169.0 - 2026-11-13

### Runtime

- Preserve bounded native websocket frames upstream (`c9a7926`)

### Misc

- Retain cold-cache bootstrap with cache-first Playwright (`df2fa86`)
- Keep Playwright startup independent of npm registry (`28c454d`)

## 0.167.8 - 2026-11-12

### Docs

- Clarify native Vertex launch boundary (`193e603`)

## 0.167.7 - 2026-11-11

- No grouped changes.
