# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.167.8 - 2026-11-12

### Docs

- Clarify native Vertex launch boundary (`193e603`)
# Prodex 0.167.8

Qualify Codex CLI 0.167.2 and preserve native Vertex incremental-tool behavior.

## New Features

- Advance the tested Codex reference to 0.167.2 using pinned source and official binaries.
- Guard the stable, default-on `incremental_tools_legacy_gcp` feature and its independent opt-out. Bedrock's legacy flag is now stable but remains disabled by default.

## Bug Fixes

- Update compatibility assertions for native GCP tool routing, namespaces, tool search, history-prefix preservation, and remote compaction.
- Extend Smart Context exact-reconstruction coverage to incremental requests with an empty top-level tool catalog, including compaction, while retaining hosted-search coverage.

## Compatibility and Upgrade

Prodex preserves upstream ownership of native GCP authentication and transport
through `prodex run` with a GCP-configured profile. This qualification does not
add Vertex to the Super provider picker; that picker retains its existing
supported-provider boundary.
Regular OpenAI and custom local endpoints are not switched to GCP incremental
features. Existing local API-key handling, reasoning, resume, and compaction
behavior remains unchanged. Compatible installed RTK, Codebase Memory MCP,
Playwright MCP, and Ponytail remain automatic in Super; Presidio still asks
for consent.

Issue #127 reports local pre-commit profile selection exhaustion without a
version or diagnostic log. It is not marked fixed by this release: the screenshot
alone does not identify a safe, reproducible runtime correction. Existing bounded
recovery, quota classification, and continuation ownership safeguards remain in place.

## Changelog

- Qualify Kiro CLI 3.2.1 for the release's native-client validation without changing its minimum runtime policy.

Full Changelog: [0.167.7...0.167.8](https://github.com/christiandoxa/prodex/compare/0.167.7...0.167.8)

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

## 0.166.3 - 2026-11-01

### Misc

- Retain maintainer attribution and signed ancestry (`efbbaf1`)
- Reject reintroduction of pre-cleanup Git roots (`b0715b3`)
- Prevent incident data from returning to tracked source (`ff640fc`)
- Qualify warm tokenizer startup and stable tool integration (`d437f5e`)
- Enforce safe automatic Smart Context on all launches (`f9a8af2`)

## 0.166.2 - 2026-10-31

### Misc

- Preserve native root session ownership (`2c86952`)
