# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.167.6 - 2026-11-11

### Misc

- Refresh optional-tool release qualification (`753be7e`)
- Render local compaction warnings with terminal UI (`424e678`)
- Improve local compaction observability (`6c83994`)
# Prodex 0.167.6

Make local-model compaction easier to monitor and configure.

## New Features

- Warn when a local auto-compaction threshold leaves little effective context for tool output.
- Show local compaction start, periodic progress, and elapsed request time in `prodex log` when Codex supplies turn metadata.

## Bug Fixes

- Apply the context-headroom warning to both normal and dry-run launches.
- Cover preservation of completed tool history through local compaction and follow-up requests.

## Changelog

- Refresh the release-qualified Playwright MCP reference to 0.0.85; the existing runtime minimum remains unchanged.
- Keep local compaction diagnostics free of prompts, session identifiers, and API keys.

Full Changelog: [0.167.5...0.167.6](https://github.com/christiandoxa/prodex/compare/0.167.5...0.167.6)

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

## 0.166.1 - 2026-10-31

### Docs

- Describe the 0.166.1 optional-tool changes (`c68417d`)

### Misc

- Qualify tunnel-client 0.0.19 and isolate stdio policy (`3869d75`)
- Retire Caveman and harden automatic launch (`a7d7c6a`)

## 0.166.0 - 2026-10-30

### Docs

- Add 0.166.0 release notes (`91fae9c`)

### Misc

- Preserve Guardian parent routing affinity (`4b3c21c`)
- Accommodate Codex 0.166.0 policy responses (`5c0e7a7`)
