# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.172.2 - 2026-11-25

### CLI

- Add OpenCode Go sign-in and launch (`a2afec4`)

### Misc

- Include the Go model in the provider catalog (`1260268`)

## 0.172.1 - 2026-11-24

### Deps

- Refresh stable Rust dependency graph (`3076d5a`)

### Misc

- Align packaging with current CLI targets (`3e3c1cc`)
- Build strict Mojo images on stable dependencies (`da32122`)

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
