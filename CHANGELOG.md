# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.169.0 - 2026-11-13

### Runtime

- Preserve bounded native websocket frames upstream (`c9a7926`)

### Misc

- Retain cold-cache bootstrap with cache-first Playwright (`df2fa86`)
- Keep Playwright startup independent of npm registry (`28c454d`)
# Prodex 0.169.0

Qualify Codex CLI 0.169.0 while retaining native trust, authentication, resume,
and transport behavior in ordinary Prodex and Super launches.

## New Features

- Advance the tested Codex baseline from 0.167.2 to the pinned 0.169.0 source and official binary.
- Guard credential-source-bound request setup, runtime WebSocket reauthentication,
  prompt-cache options, and bounded multi-frame native requests.
- Refresh the app-server lifecycle schema hashes after reviewing optional moderation
  metadata, inference-hook errors, and file-backed audio alternatives.

## Bug Fixes

- Restore the native 1 MiB upstream WebSocket frame bound after request routing and redaction. Large Unicode requests remain one text message, with backpressure between frames and unchanged transport failure/affinity handling.

- Prefer cached npm metadata for default Playwright discovery, Super startup and expose aliases. This avoids redundant registry refreshes for prepared packages while retaining cold-cache bootstrap, no-install behavior and explicit user MCP configuration.

- Follow extracted upstream authentication and provider-test owners without dropping
  custom-provider credential isolation checks.
- Preserve security-notice identity and no-redirect checks after their move from the
  TUI to app-server, including the TUI's final account and HTTPS action validation.
- Add regression coverage proving Smart Context retains incremental exec definitions,
  removal notices and prompt-cache TTL settings with exact request reconstruction.
- Verify native terminal runtime, workspace and policy errors are not reclassified
  as quota, overload or continuation-recovery signals.

## Compatibility and Upgrade

Native Codex owns explicit `exec --trust`, saved-provider restoration, observed MCP
startup rounds and compaction retry policy. Prodex does not automatically grant
project trust or enable experimental incremental-exec features. Existing local
API keys, explicit model/context choices, UUID resume, `/resume`, reasoning logs
and compaction remain supported. Compatible installed RTK, Codebase Memory MCP,
Playwright MCP and Ponytail remain automatic in Super; Presidio still asks for consent.

Interactive qualification uses isolated loopback services and synthetic credentials;
it is not a claim of live-provider coverage or equivalent capabilities across models.
The supported Codex minimum remains capability-based. Update Codex separately to
0.169.0 to use the newly qualified native behavior.

## Changelog

- Qualify Kiro CLI 3.3.0 for native release checks; retain the existing runtime minimum.

Full Changelog: [0.167.8...0.169.0](https://github.com/christiandoxa/prodex/compare/0.167.8...0.169.0)

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

## 0.166.3 - 2026-11-01

### Misc

- Retain maintainer attribution and signed ancestry (`efbbaf1`)
- Reject reintroduction of pre-cleanup Git roots (`b0715b3`)
- Prevent incident data from returning to tracked source (`ff640fc`)
- Qualify warm tokenizer startup and stable tool integration (`d437f5e`)
- Enforce safe automatic Smart Context on all launches (`f9a8af2`)
