# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.167.4 - 2026-11-09

### Misc

- Support endpoint-scoped API keys for URL launches (`e6667cf`)
- Avoid duplicate cached reasoning on complete tool replays (`7f6f586`)
- Retain resume bindings and complete reasoning streams (`e1b1c89`)
# Prodex 0.167.4

Make authenticated local-model conversations resumable across launches, preserve
reasoning in Codex and transcript logs, and support API-key-protected endpoints.

## New Features

- Accept `--api-key` with `--url`, before or after a resume UUID. Support
  `PRODEX_LOCAL_API_KEY` as the local-only environment fallback; an explicit
  flag takes precedence without inheriting a hosted provider's credential.
- Authenticate local Chat Completions, generic Responses, streaming, and
  compaction requests. Keep Codex model catalog discovery local.
- Keep local keys out of generated Codex configuration, session state, normal
  child-process environments, and diagnostics. Reject malformed keys without
  echoing them, preserve authentication error status, and do not follow local
  upstream redirects.

## Bug Fixes

- Keep profileless local-adapter identities distinct from deleted user-managed
  profiles when saving runtime state and continuation sidecars.
- Permit explicit Offrail/Bonsai CLI launches to replay client-supplied history
  despite historical session-only binding tombstones. Preserve endpoint and
  credential enforcement for opaque response IDs, turn state, hosted providers,
  and gateway traffic.
- Record translated Responses IDs instead of raw Chat Completions payloads.
- Use a supplied complete local transcript directly rather than appending cached
  reasoning from earlier tool calls again. Retain exact cached history for
  delta-only tool replies and opaque response continuations.
- Open and close real reasoning items around streamed summaries. Maintain item
  identities, canonical output positions, and final ordering across reasoning,
  answers, and tool calls.
- Preserve plaintext reasoning when Codex replays a saved chat or returns tool
  results. Continue rejecting opaque encrypted reasoning and malformed content
  instead of dropping or inventing it.
- Let newly discovered log transcripts replay a bounded complete-line tail so
  short reasoning turns are not skipped between discovery ticks.

## Compatibility and Upgrade

The qualified Codex CLI remains 0.167.0. Explicit local model selection and
context/compaction budgets are preserved in fresh and resumed Super sessions,
including the owned native app-server path. RTK, Codebase Memory MCP,
Playwright MCP, and Ponytail remain automatic when installed and compatible;
Presidio remains an interactive consent choice.

Use the context capacity configured on the model server. Local OpenAI API
interoperability does not imply hosted-model capability or quality equivalence.
Cloudflare Tunnel transports the endpoint; model-server authentication and
Cloudflare Access policy are separate concerns. See [Local Models](https://github.com/christiandoxa/prodex/blob/main/LOCAL.md)
for commands, credential handling, reasoning levels, and backend expectations.

## Changelog

Full Changelog: [0.167.3...0.167.4](https://github.com/christiandoxa/prodex/compare/0.167.3...0.167.4)

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

## 0.165.1 - 2026-10-28

### Runtime

- Park idle sockets and preserve interrupt transport ownership (`6a012e6`)

### Misc

- Expose bounded controls for upgraded local sockets (`8a2bbed`)

## 0.165.0 - 2026-10-28

### CLI

- Retain Codex memory v2 across account switches (`a114606`)

### Docs

- Record Codex 0.165 qualification and numbering migration (`29caf98`)

### Deps

- Qualify Kiro CLI 2.35.1 release packaging (`9672851`)

### Misc

- Qualify Codex 0.165 native provider boundaries (`ba437ca`)
- Support the explicit Codex-aligned release transition (`84a45e6`)
