# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.169.2 - 2026-11-18

### Deps

- Replace unmaintained Chrono with a Jiff host time boundary (`cc1a6fa`)

### Misc

- Preserve persisted calendar display and timestamp contracts (`44c3391`)
- Qualify Codex 0.169.1 provider and model contracts (`e9b9fa2`)
- Add GPT-6.2 Astra and Sol without overriding native defaults (`f7437e7`)
- Repair external home rollout indexes before launch (`f60da6b`)
# Prodex 0.169.2

Qualify Codex CLI 0.169.1, add GPT-6.2 Astra and Sol, and repair external-profile
session resume after a temporary overlay has been removed.

## New Features

- Add `gpt-6.2-astra` and `gpt-6.2-sol` to the canonical OpenAI model catalog.
  Both accept text and image inputs, support 1,050,000 API context tokens and
  128,000 output tokens, and use Responses for tool calling. The Codex-facing
  effort catalog includes `low`, `medium`, `high`, `xhigh`, `max`, and `ultra`,
  with `low` as the bundled default. `ultra` is native task delegation, not a
  promise that every API/account configuration supports that mode.
- Qualify the official Codex CLI 0.169.1 package and generated app-server schemas.
  Preserve upstream's default-on Bedrock incremental tools and Mantle's narrowly
  scoped empty-commentary-annotation normalization before replay.

## Bug Fixes

- Repair stale SQLite rollout pointers in the selected persistent external or
  profileless home before UUID resume or interactive `/resume`. The verified
  transcript remains at its original path; its UUID, contents, title, and update
  time are not rewritten. Missing or mismatched transcripts are never fabricated.
- Use conservative native fallback context windows for GPT-6.2 Smart Context
  accounting: 272,000 for Astra and 500,000 for Sol. Explicit launch limits still
  take precedence. This does not enable an unproven tokenizer or override Codex
  context, compaction, or reasoning configuration.

## Compatibility and Upgrade

The qualified client is [Codex CLI 0.169.1](https://github.com/openai/codex/releases/tag/rust-v0.169.1).
The upstream patch contains model catalog additions, Bedrock incremental-tool
activation, and provider-owned Mantle replay normalization; Prodex does not
reimplement or force those provider policies.

Native Codex context defaults are not the API maximum: Astra starts at 272,000
and Sol at 500,000, while their bundled opt-in maximum is 872,000. Account model
catalogs remain authoritative for availability and reasoning levels, including
main-agent and sub-agent selections. Existing exact model IDs, established
`astra`/`sol` aliases, saved sessions, and explicit CLI configuration are retained.
No unverified Copilot or Kiro model entitlement is added.

Flat catalog prices remain unknown because they cannot faithfully represent the
published context-length, cache, and processing-tier prices. See the official
[GPT-6.2 announcement](https://openai.com/index/introducing-gpt-6-2-astra-sol/),
[Astra API metadata](https://developers.openai.com/api/docs/models/gpt-6.2-astra),
[Sol API metadata](https://developers.openai.com/api/docs/models/gpt-6.2-sol), and
[pinned native catalog](https://github.com/openai/codex/blob/c376cf7396eff83ce4c6ac61b9553df142e7a0be/codex-rs/models-manager/models.json).

The 0.169.1 fixes for untrusted `/goal` context, proxy transport, custom trust,
and persistent goal state are retained. The already-published 0.169.1 tag and
artifacts are not replaced. Native account discovery over a SOCKS-only listener
remains outside Prodex-owned inference transport qualification.

## Changelog

- Qualify the official Kiro CLI 3.4.0 stable archive, its chat and ACP command
  surfaces, and release installer fixtures; retain the existing runtime defaults.
- Replace the unmaintained registry Chrono dependency (RUSTSEC-2026-0543) with
  a Jiff-backed host time boundary. Preserve session date paths, persisted timestamp
  formats, quota reset parsing, and daylight-saving handling without suppressing
  the dependency advisory or altering runtime policy.
- Extend model, account-catalog, context-preservation, and external-home resume
  regression coverage. Keep compatibility guards pinned to the audited source
  and generated native lifecycle schemas.
- Retain RTK, Codebase Memory MCP, Playwright MCP, and Ponytail activation in
  Super, and preserve the existing Presidio consent boundary.

Full Changelog: [0.169.1...0.169.2](https://github.com/christiandoxa/prodex/compare/0.169.1...0.169.2)

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

## 0.167.0 - 2026-11-06

### CLI

- Honor the native Codex embedded transport (`4a418a2`)

### Docs

- Describe the Codex 0.167.0 upgrade (`b2a5970`)

### Misc

- Qualify Codex 0.167.0 lifecycle and request contracts (`4da2c28`)
