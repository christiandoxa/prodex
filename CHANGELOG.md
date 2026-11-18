# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.170.0 - 2026-11-18

### Runtime

- Preserve native HTTP fallback for oversized WebSocket requests (`9defc62`)

### Misc

- Qualify Codex 0.170.0 source and lifecycle contracts (`c887792`)
# Prodex 0.170.0

Qualify Codex CLI 0.170.0 and preserve its immediate HTTP fallback for oversized
Responses WebSocket requests through the Prodex runtime proxy.

## New Features

- Qualify the official Codex CLI 0.170.0 package and its generated stable
  app-server lifecycle schemas, including versioned agent-message display
  metadata and the required native RPC methods.

## Bug Fixes

- Forward an upstream WebSocket close code `1009` received before any text frame
  to native Codex without spending another transport attempt or penalizing the
  profile. Codex can immediately switch the request to HTTP.
- Honor the upstream `x-codex-websocket-max-message-bytes` handshake limit against
  the final serialized request before sending it. Oversized requests, including
  prewarming, receive the same native fallback signal without an upstream
  generation request. Invalid or absent limits preserve normal behavior.
- Keep size closes after upstream text within the existing transport recovery
  rules. Response commitment, continuation ownership, and configured proxy and
  trust settings remain authoritative across transport changes.

## Upstream Compatibility

The qualified client is [Codex CLI 0.170.0](https://github.com/openai/codex/releases/tag/rust-v0.170.0),
verified against source commit `29eb79e9866b3262a777fdf8195c1b9e63684c91` and the
official native package.

- Track upstream changes to realtime thread headers, saved model-facing ID
  allocation, compaction replay, active recovery turns, permission profiles,
  and experimental MCP environment identity in the compatibility baseline.
- Preserve native ownership of realtime WebRTC negotiation, refreshable OTLP
  headers, MCP OAuth cancellation safety, and sandbox permission normalization.
  Native feature configuration and account/provider availability still apply.

## Session and Provider Compatibility

The existing managed and external-profile resume paths, `/resume`,
`prodex resume`, `prodex s <uuid>`, and persistent `/goal` state remain covered
by local native-client acceptance. Session repairs preserve transcript content
and use the selected persistent home. GPT-6.2 Astra and Sol remain available in
the Prodex catalog with their existing native context defaults.

Proxy qualification covers HTTP/HTTPS CONNECT, SOCKS inference, authentication,
custom certificate trust, and session continuation. Native Codex account discovery
through a SOCKS-only listener remains outside the Prodex-owned inference path.
Validation uses isolated profiles and local mock providers with synthetic
credentials; it does not certify every live provider or account feature.

Prodex remains a wrapper. Install the official Codex CLI separately to use the
newly qualified native version.

## Changelog

- Validate committed native schema fixtures against their pinned hashes in the
  default offline compatibility gate, with stale-fixture regression coverage.
- Add WebSocket size-boundary checks and native-client acceptance for fresh and
  UUID-resumed HTTP fallback while retaining the existing proxy and session tests.

Full Changelog: [0.169.2...0.170.0](https://github.com/christiandoxa/prodex/compare/0.169.2...0.170.0)

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

## 0.167.5 - 2026-11-09

### Deps

- Bump toml (`59ec2b3`)

### Misc

- Preserve unkeyed Responses session bindings (`e3a7a5d`)
- Merge remote-tracking branch 'origin/pr-125' into task/strata-01675 (`c1839ef`)
- Merge remote-tracking branch 'origin/pr-124' into task/strata-01675 (`9cf3bbe`)

## 0.167.4 - 2026-11-09

### Misc

- Support endpoint-scoped API keys for URL launches (`e6667cf`)
- Avoid duplicate cached reasoning on complete tool replays (`7f6f586`)
- Retain resume bindings and complete reasoning streams (`e1b1c89`)

## 0.167.3 - 2026-11-08

### Docs

- Keep the release guide link valid in generated changelogs (`9e3bcc9`)

### Misc

- Preserve model selection and support Offrail coding tools (`cf22e35`)

## 0.167.2 - 2026-11-08

### Misc

- Stream large legacy histories and bound startup reads (`f6e8647`)

## 0.167.1 - 2026-11-08

### Runtime

- Preserve upstream proxy routing during account discovery (`5b70596`)

### CLI

- Preserve native OpenAI session identity (`0263c42`)

### Misc

- Flush retained backups with writable Windows handles (`db90906`)
- Add bounded legacy OpenAI identity repair (`eb7cf48`)
