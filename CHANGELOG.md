# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.172.0 - 2026-11-24

### Misc

- Qualify native 0.172 compatibility (`88f4cbf`)
# Prodex 0.172.0

Qualify Codex CLI 0.172.0 and preserve native session, stream, and tool behavior.

## New Features

- Support the native Codex 0.172.0 session lifecycle, including saved conversation
  links, slash-command argument completion, and agent command-center navigation.
- Qualify saved permissions, working directories, and workspace roots for
  `prodex exec resume`, together with cross-platform remote session paths.
- Preserve native lazy startup for cached optional stdio MCP servers, per-tool
  app approval reviewers, and the gRPC Code Mode host.

## Bug Fixes

- Classify `api_key_spend_limit_exceeded` as explicit quota exhaustion across
  HTTP, SSE, and WebSocket routes. Eligible rotation remains limited to the
  pre-commit phase; existing output and hard session affinity remain protected.
- Upgrade `url` to 2.5.9 and its required `form_urlencoded` dependency to 1.2.3
  to resolve RUSTSEC-2026-0592 without refreshing unrelated dependencies.

## Windows Scan Review

The Windows x64 artifact retains a reviewed ClamAV `Win.Trojan.Virut-225`
false positive in unchanged expression-destruction code. Acceptance is bound
to this version and its exact SHA-256, with source, artifact and smoke checks;
all other findings and scanner errors still block release.
See the [review and exact hash](https://github.com/christiandoxa/prodex/blob/0.172.0/docs/supply-chain.md).

## Upstream Compatibility

The standalone wrapper uses a separately installed [Codex CLI 0.172.0](https://github.com/openai/codex/releases/tag/rust-v0.172.0).
Qualification uses the checksum-verified official Linux binary and source commit
`3d86e43b075adeba2193ec5c22ac92f55afd82e6`.

## Changelog

- Refresh 13 native lifecycle schema hashes and preserve all 12 required RPC
  methods. Seven schemas add distinct API-key, project, and organization
  spend-limit error identities.
- Extend source qualification to immediate stream errors, saved resume settings,
  cross-platform paths, and native connector-result ownership and privacy gates.
- Retain the GPT-6.2 Sol/Astra catalog, proxy routing, session resume forms, and
  Ornith goal-context adaptation from previous releases.

Full Changelog: [0.171.3...0.172.0](https://github.com/christiandoxa/prodex/compare/0.171.3...0.172.0)

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

## 0.167.6 - 2026-11-11

### Misc

- Refresh optional-tool release qualification (`753be7e`)
- Render local compaction warnings with terminal UI (`424e678`)
- Improve local compaction observability (`6c83994`)
