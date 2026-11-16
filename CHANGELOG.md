# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

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
# Prodex 0.169.1

Respect outbound proxy and certificate settings across native transports and session
resume; preserve native goal context on local Chat Completions adapters.

## New Features

- Qualify native WebSocket forwarding through authenticated SOCKS5 and HTTPS proxies
  while retaining endpoint-aware bypass rules and certificate validation.

## Bug Fixes

- Support HTTP CONNECT, HTTPS proxy tunnels, and SOCKS5/SOCKS5h for native WebSockets,
  including proxy authentication and bounded negotiation. Preserve explicit proxy
  bypass and `NO_PROXY`; never retry a failed configured proxy by silently dialing direct.
- Apply `CODEX_CA_CERTIFICATE`, falling back to `SSL_CERT_FILE`, consistently to upstream
  HTTP and WebSockets without disabling certificate verification.
- Scope reusable runtime brokers to an opaque fingerprint of outbound proxy, bypass,
  and custom trust settings. A new launch cannot inherit a stale broker's proxy;
  existing sessions on that broker are left intact.
- Keep SQLite session indexes, queues, and goals in the persistent source home for
  external/profileless Super launches as well as managed profiles. A disposable
  overlay no longer loses newly created goal state or hides saved sessions.
- Preserve named, unpaired native context such as `goal_objective` as untrusted user-level
  context when translating Responses to Chat Completions. Its item ID is not a tool-call
  owner. Full-history local goal continuation no longer prepends cached conversation
  items or duplicates reasoning; real delta-only tool replies retain history recovery.

## Compatibility and Upgrade

The qualified native client remains Codex CLI 0.169.0. UUID resume, `prodex resume`,
interactive `/resume`, local API-key authentication, explicit model/context selection,
reasoning logs, and compaction retain their existing contracts. Session history is not
rewritten or truncated by these changes. RTK, Codebase Memory MCP, Playwright MCP,
and Ponytail remain automatic in Super; Presidio still asks for consent.

SOCKS transport qualification covers Prodex-owned inference and session continuation. Codex 0.169.0's own account-discovery HTTP client does not enable
SOCKS support; ChatGPT discovery through a SOCKS-only listener remains an upstream
limitation. Use the proxy's HTTP CONNECT listener for that native login path.
Prodex does not bypass the selected proxy to conceal an unsupported upstream client.

Issue #128 reports a pre-commit failure without its exact proxy configuration or
transport log. This release addresses independently reproduced proxy compatibility
gaps; the reporter's deployment still needs confirmation with the updated version.

## Changelog

- Qualify official tunnel-client 0.0.21; isolate its new ambient residency, OAuth-discovery,
  and admin-UI settings without clearing proxy or bypass variables.

- Integrate Dependabot #129 and #130: arc-swap 1.9.5, rpassword 7.5.5, rusqlite 0.42.0,
  uuid 1.29.2, zstd 0.14.4, and their reviewed lockfile updates, including fuzz metadata.
- Integrate Dependabot #131: SonarQube scan action 8.5.0, pinned to its immutable commit.
- Integrate Dependabot #132: refresh the Rust 1.99.1 Bookworm image digest without
  changing the selected Rust compiler version.

Full Changelog: [0.169.0...0.169.1](https://github.com/christiandoxa/prodex/compare/0.169.0...0.169.1)

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

## 0.167.0 - 2026-11-06

### CLI

- Honor the native Codex embedded transport (`4a418a2`)

### Docs

- Describe the Codex 0.167.0 upgrade (`b2a5970`)

### Misc

- Qualify Codex 0.167.0 lifecycle and request contracts (`4da2c28`)

## 0.166.4 - 2026-11-05

### Runtime

- Remove default request body size ceiling (`616792a`)
- Preserve large payloads during request handling (`d59e47e`)
