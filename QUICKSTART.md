# Prodex Quickstart

This path imports one Codex login, verifies quota, and launches Codex through
Prodex. The current local version in this repo is `0.425.0`.

## 1. Install prerequisites

You need:

- a supported macOS, Linux, or Windows host;
- Codex CLI available as `codex` on `PATH`;
- one logged-in Codex account for quota-aware routing.

Windows must permit symbolic-link creation for managed profiles. Enable
Developer Mode, grant `SeCreateSymbolicLinkPrivilege`, or run Prodex as an
administrator before importing or creating a profile.

Install a published release using the
[download, inspect, and verify instructions in the README](README.md#installation).

After the installer completes:

```sh
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
prodex login second
```

Prodex keeps each managed profile in a separate Codex home.

## 3. Verify state and quota

```bash
prodex profile list
prodex current
prodex quota --once
prodex doctor
```

Bare `prodex quota` shows the detailed pool view. Without `--once`, human quota
views refresh every five seconds. `--raw` is always one-shot.

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

Check every configured eligible OpenAI profile with isolated minimal model
requests:

```bash
prodex ping openai
```

Each probe sends user text `ping`, stays pinned to its profile, and continues
after another profile fails. `--profile NAME` limits the check to one profile;
`--json` emits the aggregate per-profile result. A valid completed response is
enough; exact `PONG` wording is not required. This is an application-level
probe, not a DNS or server-health probe. For expose modes, use
`prodex s expose` for local-only browser/MCP access, `--tunnel` for the
Cloudflare Quick Tunnel, or `--tunnel-provider openai` for MCP-only OpenAI
Secure Tunnel access; the browser remains loopback-local in OpenAI mode, and
tunnel-client readiness does not by itself verify ChatGPT connector creation.

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
prodex s --sub-agent --sub-agent-max-concurrency default
prodex s --sub-agent --sub-agent-max-concurrency 8
prodex s --presidio --sub-agent --sub-agent-provider kiro \
  --sub-agent-model gpt-5.6-luna --sub-agent-model-reasoning-effort max \
  --sub-agent-max-concurrency 16
prodex s 00000000-0000-7000-8000-000000000042 --presidio --sub-agent \
  --sub-agent-max-concurrency=23
```

Interactive order is Presidio, main-agent provider, required main-provider
configuration, sub-agent opt-in, then child provider, local URL when needed,
model, catalog-backed or standard effort, and maximum active children. Explicit
`--sub-agent` skips that child wizard and uses OpenAI/provider defaults.

The default maximum is 4; presets are 4, 8, 16, and 32; custom values accept 1
through 64. OS-backed exclusive slots enforce this across separate official
launcher processes. The child uses the current Prodex executable, a private
bounded task file, `--no-sub-agent`, exactly one inherited Presidio flag, and a
shell-free argument vector. Parent UUIDs are never inherited. See
[Sub-agents](docs/sub-agents.md) for resume affinity, model catalogs,
limit-reached behavior, instruction injection, dry-run output, and MVP limits.

## 6. Connect ChatGPT to Super

From the workspace you want to expose:

```bash
prodex s expose
```

Interactive setup asks for the main agent, main model, model-aware reasoning
effort, and optional sub-agent model/effort configuration before starting any
listener or Cloudflare process. In a headless shell, use existing options such
as `--model` and `-c 'model_reasoning_effort="max"'`; explicit values win and
stdin is never read indefinitely.

After MCP readiness is verified, paste the printed URL into ChatGPT:

```text
Settings → Security and login → Developer mode → Plugins → + → Public MCP server URL
```

With one plain `prodex s` already running in this workspace, MCP also exposes
the existing-session bridge by default. Call
`prodex_session_prompt_inject({"message":"inspect the failing test"})`, then
call `prodex_session_output_read({})`. Save `next_cursor` and pass it as
`cursor` on the next read; output is bounded and never consumes the TUI stream.
These tools use the same proven Codex thread and do not start another solver.

Cloudflare mode prints a public URL ending in `/mcp` and containing a fresh
ephemeral full-Super capability.
Anyone with the full URL can control that expose process, so treat it as a
credential. This is not OAuth and is for personal development only. No
Cloudflare account or initialization is required for Quick Tunnel mode, but
`cloudflared` must be installed. Quick Tunnel prefers QUIC over outbound
UDP/7844 and falls back to HTTP/2 over TCP/7844. Local mode has no external
tunnel. OpenAI mode requires a pre-created tunnel ID, the
`CONTROL_PLANE_API_KEY` runtime key, and the official `tunnel-client`; it uses
outbound HTTPS/TCP 443 and provides MCP connectivity only. The browser remains
local in OpenAI mode. Stop the process with Ctrl+C to revoke access.

To use parallel workspaces, create separate worktrees and run one process in
each; every process has its own port, hostname, capability, server identity,
configuration, and run manager:

```bash
git worktree add ../feature-a -b feature/a
git worktree add ../feature-b -b feature/b
cd ../feature-a && prodex s expose --name feature-a
cd ../feature-b && prodex s expose --name feature-b
```

See [Expose](EXPOSE.md) for route isolation, tool lifecycle,
workspace binding, and security limits.

## 7. Select another provider

Examples:

```bash
GEMINI_API_KEY=example-key prodex s gemini
# Vertex AI is supported through the native Gemini CLI, which owns auth and transport:
prodex s gemini --cli gemini

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

Use `log_path` from the JSON output to inspect an explicitly recorded runtime
log. Live `prodex log stream` and `prodex log upstream` use the bounded
authenticated live runtime sources, including direct and broker-backed
proxies, by default, so normal observability does not grow
a raw disk journal. Runtime notices are never printed over the Codex TUI.
`prodex audit` exposes local events; it is not immutable compliance retention or
a disaster-recovery plan.

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
