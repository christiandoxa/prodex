# Prodex Quickstart

This path imports one Codex login, verifies quota, and launches Codex through
Prodex. The current local version in this repo is `0.382.0`.

## 1. Install prerequisites

You need:

- a supported macOS, Linux, or Windows host;
- Codex CLI available as `codex` on `PATH`;
- one logged-in Codex account for quota-aware routing.

Windows must permit symbolic-link creation for managed profiles. Enable
Developer Mode, grant `SeCreateSymbolicLinkPrivilege`, or run Prodex as an
administrator before importing or creating a profile.

Install Prodex on macOS or Linux:

```bash
curl -fsSL https://github.com/christiandoxa/prodex/releases/latest/download/install.sh | sh
prodex --version
codex --version
```

Windows PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/christiandoxa/prodex/releases/latest/download/install.ps1 | iex"
prodex --version
codex --version
```

## 2. Import or create a profile

Import the current Codex login:

```bash
prodex profile import-current main
```

If no current login exists, create one:

```bash
prodex login
```

Add another account only when needed:

```bash
prodex profile add second
prodex login --profile second
```

Prodex keeps each managed profile in a separate Codex home.

## 3. Verify state and quota

```bash
prodex profile list
prodex current
prodex quota --all --once
prodex doctor
```

Without `--once`, human quota views refresh every five seconds. `--raw` is
always one-shot.

## 4. Launch

Interactive Codex:

```bash
prodex
```

One prompt:

```bash
prodex exec "review this repository"
```

Explicit profile:

```bash
prodex run --profile second
```

Prodex preserves continuation affinity and rotates only before commit. It does
not move a live continuation or rotate after streamed output begins.

## 5. Use Super/YOLO mode

Inspect the plan first:

```bash
prodex s --dry-run
prodex capability super-doctor
```

Then launch:

```bash
prodex s
```

Super is the explicit YOLO path: it bypasses approvals and the sandbox, bypasses
hook-trust confirmation, and trusts the current workspace for that invocation.
It asks whether to enable Presidio before an interactive launch. Answer without
a prompt when wanted:

```bash
prodex s --presidio
prodex s --no-presidio
```

Use `prodex run` when normal Codex approvals and workspace-trust prompts are
preferred.

Optional tools are typed and session-local:

```bash
prodex super --tool rtk --tool ponytail
prodex super --require-tool caveman
```

Missing tools are skipped unless required. Caveman is external, never embedded
or downloaded at launch. Follow [Optional Tools](docs/optional-tools.md) before
using:

```bash
prodex caveman --dry-run
prodex caveman
prodex claude caveman
```

Super sub-agents accept fresh and resumed Codex targets:

```bash
prodex s --sub-agent --no-presidio
prodex s --presidio --sub-agent --sub-agent-provider kiro \
  --sub-agent-model gpt-5.6-luna --sub-agent-model-reasoning-effort max
prodex s --sub-agent --sub-agent-provider local \
  --sub-agent-url http://127.0.0.1:11434/v1 --sub-agent-model example-local-model \
  --no-presidio
prodex s 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex s 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9 --presidio --sub-agent \
  --sub-agent-provider kiro --sub-agent-model gpt-5.6-luna \
  --sub-agent-model-reasoning-effort max
```

The child uses `prodex s --no-sub-agent`, an explicit inherited Presidio choice,
the selected provider/optional model, then optional `-c model_reasoning_effort=<value>`,
then `exec <shell-safe-task>` in that order. OpenAI omits the provider flag;
local uses `--url`; external providers use `--provider`. Parent UUIDs are never
forwarded to children. Explicit `--sub-agent` skips the provider/model/effort
wizard and uses provider defaults. See
[Sub-agents](docs/sub-agents.md) for flags before/after a UUID, `--` handling,
the recursion marker, all 17 generated `SUB_AGENTS.md` rules, TTY/non-TTY and
dry-run behavior, native frontend rejection, bounded prompts, and overlay
session access. The canonical providers are `openai`, `anthropic`, `copilot`,
`deepseek`, `gemini`, `kiro`, and `local`; the public Kiro example requests
`gpt-5.6-luna`/`max`, but the provider decides whether that model and effort
are supported. Custom model IDs and documented reasoning-effort values are
optional and are omitted when unset rather than copied from the parent. The interactive local URL
prompt is prefilled with `http://127.0.0.1:11434/v1`; explicit and non-TTY
local launches still require a URL. URL validation rejects credentials,
queries, and fragments but does not prove endpoint trust or compatibility.
Interactive order is Presidio, sub-agent opt-in, provider, model, effort, then
local URL; non-TTY defaults disable both prompts. Native frontends reject the
bridge. Parent UUIDs are not inherited, and local subprocess/A2A extension
details are in the guide.

## 6. Select another provider

Examples:

```bash
prodex login --with-google
prodex s gemini

prodex login --with-claude
prodex s --provider anthropic

DEEPSEEK_API_KEY=example-key prodex s deepseek

prodex s --url http://127.0.0.1:8131 --model local-model
```

Provider routes and feature support differ; adapter labels do not guarantee
complete native-provider fidelity. Check the generated
[provider matrix](docs/provider-capabilities.md) and local-model notes in
[LOCAL.md](LOCAL.md).

## 7. Claude Code front end

```bash
prodex claude -- -p "summarize this repository"
prodex claude --profile second -- -p "show the latest diff"
```

Claude Code talks through Prodex's Anthropic-compatible local boundary while
profile affinity and pre-commit rotation stay Prodex-owned.

## 8. Diagnose a problem

```bash
prodex doctor --runtime
prodex doctor --runtime --json
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex audit --tail 20
```

Use `log_path` from the JSON output to inspect the active runtime log. Runtime
notices are never printed over the Codex TUI. `prodex audit` exposes local
events; it is not immutable compliance retention or a disaster-recovery plan.

If Prodex returns `409 stale_continuation`, resume with the original profile or
start a new prompt. Prodex refuses an ambiguous cross-profile replay.

If an optional tool is missing:

```bash
prodex capability super-doctor
prodex capability super-doctor --strict
```

If a launch choice is unclear:

```bash
prodex run --help
prodex super --help
prodex claude --help
```

## Next references

- [README](README.md): product overview and safe defaults.
- [Optional Tools](docs/optional-tools.md): exact external-tool contract.
- [Smart Context](docs/smart-context.md): exact/shadow semantics and evidence.
- [Sub-agents](docs/sub-agents.md): Super sub-agent command, rules, and execution boundaries.
- [Runtime Policy](docs/runtime-policy.md): configuration and environment keys.
- [Testing](docs/testing.md): contributor and operator checks.
- [Documentation Index](docs/README.md): canonical document lifecycle.
