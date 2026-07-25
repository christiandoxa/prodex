# Prodex

[![Release malware scan](https://github.com/christiandoxa/prodex/actions/workflows/standalone-release.yml/badge.svg?branch=main&event=workflow_dispatch)](https://github.com/christiandoxa/prodex/actions/workflows/standalone-release.yml)

Prodex is a local-first Codex wrapper for multiple accounts and provider
backends. It keeps continuation ownership stable, rotates eligible profiles
only before a request commits, and leaves streaming behavior to Codex.

Use Prodex when you need one or more of these:

- isolated Codex account profiles;
- quota-aware OpenAI profile selection and safe auto-rotation;
- Codex or Claude Code front ends against supported provider adapters;
- a local OpenAI-compatible gateway with governed routing;
- optional, validated developer tools in temporary launch overlays.

If one Codex account already meets your needs, direct Codex is simpler.

## Safety contract

Prodex preserves these runtime rules:

- `previous_response_id`, turn-state, and session affinity stay with their
  owning profile;
- retry or rotation happens only before unary or streaming commit;
- output never rotates mid-stream;
- upstream status, bodies, and stream payloads pass through unless Prodex
  itself failed before an upstream response existed;
- runtime notices go to logs while the Codex TUI is active;
- request hot paths do not fetch tools or dependencies.

See [architecture](docs/architecture.md), [state model](docs/state-model.md),
and [runtime policy](docs/runtime-policy.md) for the complete contracts.

## Supported providers

| Provider | Typical launch | Authentication |
| --- | --- | --- |
| OpenAI / Codex | `prodex`, `prodex run`, `prodex s` | ChatGPT OAuth, device code, or API key |
| Google Gemini | `prodex s gemini` | Google OAuth or Gemini API key |
| Anthropic | `prodex s --provider anthropic` | Claude OAuth or Anthropic API key |
| GitHub Copilot | `prodex s --provider copilot` | Imported Copilot profile or API key |
| Kiro | `prodex s --provider kiro` | Imported Kiro profile |
| DeepSeek | `prodex s deepseek` | DeepSeek API key |
| Local OpenAI-compatible | `prodex s --url http://127.0.0.1:8131` | Server-owned |

Capabilities vary by route and provider. The generated
[provider matrix](docs/provider-capabilities.md) is canonical.

## Install

Install the standalone release on macOS or Linux:

```bash
curl -fsSL https://github.com/christiandoxa/prodex/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/christiandoxa/prodex/releases/latest/download/install.ps1 | iex"
```

Install Codex separately and ensure `codex` is on `PATH`. The npm wrapper pins
one vetted Codex release; standalone Prodex uses the selected external Codex
binary. npm and crates.io are not Prodex release channels.

The current local version in this repo is `0.347.0`.

## First run

Import an existing Codex login:

```bash
prodex profile import-current main
prodex profile list
prodex quota --all --once
prodex
```

Or create two profiles:

```bash
prodex login
prodex profile add second
prodex login --profile second
prodex quota --all --once
```

Human quota views refresh every five seconds. Use `--once` for a snapshot or
`--raw` for one-shot machine-readable output.

Useful launch forms:

```bash
prodex exec "review this repository"
prodex run --profile second
prodex s
prodex s gemini
prodex s --provider anthropic
prodex claude -- -p "summarize this repository"
```

See [Quickstart](QUICKSTART.md) for one complete setup path.

## Profiles and sessions

```bash
prodex profile list
prodex current
prodex use --profile main
prodex session list
prodex quota --all
```

Profile homes keep account credentials separate. Continuations remain bound to
the profile that created them. A `409 stale_continuation` is a fail-closed
signal: start a new prompt or resume on the original profile; Prodex will not
replay an ambiguous chain on another account.

## Super and optional tools

`prodex s` and `prodex super` are the explicit Super/YOLO entrypoints. They use
typed optional-tool selection, launch Codex with approval and sandbox bypass,
bypass hook-trust confirmation, and trust the current workspace for that
invocation without changing the user's persisted Codex config.

```bash
prodex s --dry-run
prodex super --tool rtk --tool ponytail
prodex super --require-tool caveman
prodex super --presidio
prodex capability super-doctor
```

Super does not show a Presidio opt-in prompt; redaction stays disabled unless
`--presidio` is passed. Use `prodex run` instead when approval prompts and the
normal Codex workspace-trust flow are desired.

Supported optional-tool identities include Caveman, RTK, Codebase Memory MCP,
Playwright MCP, Ponytail, and Presidio. Resolution is side-effect-free;
activation only changes a temporary overlay. Missing tools are skipped unless
named by `--require-tool`.

Caveman is not embedded in Prodex. `prodex caveman` requires a versioned,
validated external installation and fails before TUI launch when it is absent.
`prodex super` skips absent Caveman safely. Installation layout, vetted digest,
compatibility fallback, and Claude activation are documented in
[Optional Tools](docs/optional-tools.md).

## Smart Context

Smart Context is built in but conservative:

- explicit exact mode is byte-for-byte pass-through;
- canary-out and disabled requests bypass parsing and mutation;
- shadow requests return original bytes and never change live or persistent
  Smart Context state;
- active rewrites require supported tokenizer counts, preserved protocol and
  critical signals, and net savings above the safety margin;
- no generated message impersonates a user;
- unresolved mandatory references never go upstream.

Current deterministic evidence covers only the inputs-only replay corpus.
Prodex makes no broader latency or task-quality claim. See
[Smart Context](docs/smart-context.md) and the generated
[replay report](docs/generated/smart-context-replay-report.md).

## Gateway

Start a loopback OpenAI-compatible gateway:

```bash
PRODEX_GATEWAY_TOKEN=change-me GEMINI_API_KEY=example-key \
  prodex gateway --provider gemini

curl http://127.0.0.1:4000/v1/responses \
  -H "Authorization: Bearer ${PRODEX_GATEWAY_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"model":"prodex-fast","input":"hello"}'
```

Non-loopback listeners require authentication. Production deployments use
projected secret references and explicit policy; raw credential URLs fail
closed. See [deployment](docs/deployment.md),
[runtime policy](docs/runtime-policy.md), [threat model](docs/threat-model.md),
and [provider conformance](docs/provider-conformance.md).

## Diagnostics

```bash
prodex doctor
prodex doctor --runtime --json
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex capability super-doctor
prodex audit --tail 20
prodex cleanup
```

Runtime logs live in the OS temporary directory by default. Override with
`PRODEX_RUNTIME_LOG_DIR` or `runtime.log_dir` in `policy.toml`. Use the
`log_path` from `prodex doctor --runtime --json`; do not guess the active file.

For stalls, inspect admission, affinity, transport, first-chunk, and state-save
markers before changing proxy behavior. The operator checklist lives in
[Testing](docs/testing.md).

## Command discovery

```bash
prodex --help
prodex <command> --help
prodex capability list
prodex info
```

CLI help is the canonical exhaustive command reference. Stable entry points
include `login`, `profile`, `run`, `exec`, `quota`, `session`, `super`,
`caveman`, `claude`, `gateway`, `doctor`, `audit`, `context`, `cleanup`, and
`update`.

## Documentation

- [Documentation index and lifecycle](docs/README.md)
- [Quickstart](QUICKSTART.md)
- [Architecture](docs/architecture.md)
- [Optional tools](docs/optional-tools.md)
- [Smart Context](docs/smart-context.md)
- [Runtime policy](docs/runtime-policy.md)
- [Testing and benchmarks](docs/testing.md)
- [Supply chain](docs/supply-chain.md)
- [Migration notes](docs/migrations/0.346-optional-tools.md)
- [Local model setup](LOCAL.md)

## Development

```bash
cargo fmt --all -- --check
cargo test -q -p prodex-runtime-proxy smart_context -- --test-threads=1
cargo test -q -p prodex-app --lib smart_context -- --test-threads=1
npm ci
npm test
npm run ci:preflight
```

Repository-wide commands and specialized security/performance gates are listed
in [Testing](docs/testing.md). Releases publish standalone GitHub assets only;
do not publish this workspace to npm or crates.io.

## License

Apache-2.0. See [LICENSE](LICENSE).
