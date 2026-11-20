# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.171.1 - 2026-11-20

### Misc

- Tolerate long-context inference prefill (`fc559d5`)
# Prodex 0.171.1

Keep long local inference requests open while the model processes the prompt.

## New Features

- Local `prodex s --url` launches with at least 131,072 context tokens now allow
  15 minutes of SSE idle time.

## Bug Fixes

- Prevent reconnects at the five-minute mark during long local prefills by
  extending the idle timeout in both Codex and the Prodex runtime proxy.

## Changelog

- Preserve explicit provider timeouts and runtime proxy policy or environment
  overrides.

Full Changelog: [0.171.0...0.171.1](https://github.com/christiandoxa/prodex/compare/0.171.0...0.171.1)

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
