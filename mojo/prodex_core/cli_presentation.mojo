from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr

comptime CLI_PRESENTATION_ABI: Int64 = 1
comptime CLI_PRESENTATION_HELP: Int64 = 0
comptime CLI_PRESENTATION_LABEL: Int64 = 1

comptime CLI_HELP_TOP_LEVEL: Int64 = 0
comptime CLI_HELP_PROFILE: Int64 = 1
comptime CLI_HELP_LOGIN: Int64 = 2
comptime CLI_HELP_QUOTA: Int64 = 3
comptime CLI_HELP_REDEEM: Int64 = 4
comptime CLI_HELP_RUN: Int64 = 5
comptime CLI_HELP_SUPER: Int64 = 6
comptime CLI_HELP_DOCTOR: Int64 = 7
comptime CLI_HELP_SESSION: Int64 = 8
comptime CLI_HELP_COUNT: Int64 = CLI_HELP_SESSION + 1

comptime CLI_COMMAND_PROFILE: Int64 = 0
comptime CLI_COMMAND_USE_PROFILE: Int64 = 1
comptime CLI_COMMAND_CURRENT: Int64 = 2
comptime CLI_COMMAND_INFO: Int64 = 3
comptime CLI_COMMAND_STATUS: Int64 = 4
comptime CLI_COMMAND_LOG: Int64 = 5
comptime CLI_COMMAND_SESSION: Int64 = 6
comptime CLI_COMMAND_DOCTOR: Int64 = 7
comptime CLI_COMMAND_LOGIN: Int64 = 8
comptime CLI_COMMAND_LOGOUT: Int64 = 9
comptime CLI_COMMAND_UPDATE: Int64 = 10
comptime CLI_COMMAND_QUOTA: Int64 = 11
comptime CLI_COMMAND_REDEEM: Int64 = 12
comptime CLI_COMMAND_PING: Int64 = 13
comptime CLI_COMMAND_RUN: Int64 = 14
comptime CLI_COMMAND_SUPER: Int64 = 15
comptime CLI_COMMAND_GATEWAY: Int64 = 16
comptime CLI_COMMAND_SUPER_EXPOSE: Int64 = 17
comptime CLI_COMMAND_RUNTIME_BROKER: Int64 = 18
comptime CLI_COMMAND_MCP_JSONL_BRIDGE: Int64 = 19
comptime CLI_COMMAND_SUB_AGENT_EXEC: Int64 = 20
comptime CLI_COMMAND_COUNT: Int64 = CLI_COMMAND_SUB_AGENT_EXEC + 1


def cli_literal(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


# Explicit LF escapes preserve Rust help bytes in CRLF source checkouts.
def cli_help_text(kind: Int64) -> ProdexRichStringView:
    if kind == CLI_HELP_TOP_LEVEL:
        return cli_literal(StringSlice(
            "Tips:\n"
            "  Bare `prodex` invocation defaults to `prodex run`.\n"
            "  Use `prodex status` for the live quota, token, and resource dashboard.\n"
            "  Use `prodex quota` for the detailed live quota view across profiles.\n"
            "  Use `prodex <command> -h` to see every parameter for that command.\n\n"
            "Examples:\n"
            "  prodex\n"
            "  prodex exec \"review this repo\"\n"
            "  prodex profile list\n"
            "  prodex status\n"
            "  prodex quota\n"
            "  prodex redeem main\n"
            "  prodex run --profile main"
        ))
    if kind == CLI_HELP_PROFILE:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex profile list\n"
            "  prodex profile add main --activate\n"
            "  prodex profile add main --insecure\n"
            "  prodex profile export\n"
            "  prodex profile export backup.json\n"
            "  prodex profile import backup.json\n"
            "  prodex profile import backup.json --insecure\n"
            "  prodex profile import claude\n"
            "  prodex profile import copilot\n"
            "  prodex profile import kiro\n"
            "  prodex profile import copilot --name copilot-main --activate\n"
            "  prodex profile import-current main\n"
            "  prodex profile remove main\n"
            "  prodex profile remove --all\n\n"
            "Notes:\n"
            "  `--insecure` bypasses local private-file permission and ACL validation; use it only with trusted files.\n"
            "  `prodex profile import kiro` reads the installed Kiro CLI auth store and profile metadata.\n"
            "  Prodex prefers `kiro-cli-chat`, falls back to `kiro-cli`, and accepts `PRODEX_KIRO_BIN` for an explicit path override."
        ))
    if kind == CLI_HELP_LOGIN:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex login\n"
            "  prodex login main\n"
            "  prodex login --profile main\n"
            "  prodex login --device-auth\n"
            "  prodex login --with-claude\n"
            "  prodex login --with-antigravity\n\n"
            "Notes:\n"
            "  A leading non-option argument selects the profile; `status` remains Codex login status.\n"
            "  Use --profile when selecting a profile literally named `status`.\n"
            "  OpenAI/Codex, Claude, and API-key login paths create or update Prodex profiles.\n"
            "  Google Gemini OAuth profiles are unsupported; native Gemini CLI / Vertex AI compatibility is retired. Use a Gemini API key with `prodex s gemini`; native Antigravity remains available through `prodex s gemini --cli agy`.\n"
            "  Antigravity login delegates to `agy auth login` and does not create a Prodex profile."
        ))
    if kind == CLI_HELP_QUOTA:
        return cli_literal(StringSlice(
            "Best practice:\n"
            "  `prodex quota` defaults to the detailed live view across all profiles.\n\n"
            "Examples:\n"
            "  prodex quota\n"
            "  prodex quota --once\n"
            "  prodex quota --provider openai\n"
            "  prodex quota --profile main --detail\n"
            "  prodex quota --all --detail\n"
            "  prodex quota --all --provider deepseek --base-url https://api.deepseek.com --once\n"
            "  prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once\n"
            "  prodex quota --all --once\n"
            "  prodex quota --all --auth no-auth --once\n"
            "  prodex quota --raw --profile main\n\n"
            "Notes:\n"
            "  `prodex quota` supports OpenAI/Codex, Anthropic OAuth, imported Copilot, DeepSeek API-key, Antigravity CLI, local OpenAI-compatible, and custom provider snapshots.\n"
            "  Use `--provider` to filter the default pool view by provider: `openai`, `gemini`, `anthropic`, `claude`, `copilot`, `kiro`, `deepseek`, `local`, or `agy`.\n"
            "  Use `--auth` to filter the default pool view by auth label or compatibility, for example `no-auth` or `quota-compatible`.\n"
            "  Explicit `--all` without `--detail` preserves the compact aggregate view.\n"
            "  If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock` or `amazon-bedrock-runtime`, prodex shows a provider snapshot instead of failing the quota view."
        ))
    if kind == CLI_HELP_REDEEM:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex redeem main\n"
            "  prodex redeem user_example.com\n"
            "  prodex redeem main --yes\n"
            "  prodex redeem main --base-url https://chatgpt.com/backend-api\n\n"
            "Notes:\n"
            "  Manual redeem sends one explicit reset-credit consume request for the named OpenAI/Codex profile.\n"
            "  If the 5h or weekly quota window resets within 1 hour, prodex asks before consuming a credit; pass `--yes` to skip that prompt.\n"
            "  The backend decides whether a credit can be applied and may return nothing-to-reset, no-credit, or already-redeemed."
        ))
    if kind == CLI_HELP_RUN:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex\n"
            "  prodex run\n"
            "  prodex super\n"
            "  prodex exec \"review this repo\"\n"
            "  prodex run --profile main\n"
            "  prodex run --web-search indexed\n"
            "  prodex run --rollout-budget-tokens 100000\n"
            "  prodex run --current-time-reminder\n"
            "  prodex run --respect-system-proxy\n"
            "  prodex run exec \"review this repo\"\n"
            "  prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9\n\n"
            "Notes:\n"
            "  Eligible pre-commit rotation is allowed by default when another supported profile or key is available.\n"
            "  Bare `prodex <args>` is treated as `prodex run <args>`.\n"
            "  A lone session id is forwarded as `codex resume <session-id>`.\n"
            "  Codex runtime feature overrides are passed through with `-c`: `--web-search disabled|cached|indexed|live`, `--rollout-budget-tokens`, `--current-time-reminder`, and `--respect-system-proxy` / `--no-respect-system-proxy`.\n"
            "  If the selected profile's `config.toml` sets `model_provider` to a non-OpenAI backend, prodex launches Codex directly without quota preflight or the local auto-rotate proxy."
        ))
    if kind == CLI_HELP_SUPER:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex super\n"
            "  prodex super --model gpt-5.3-codex\n"
            "  prodex super --url http://127.0.0.1:8131\n"
            "  prodex super deepseek --model deepseek-v4-pro\n"
            "  prodex super gemini\n"
            "  prodex super gemini\n"
            "  prodex super --provider copilot\n"
            "  prodex super --provider kiro --profile kiro-main\n"
            "  prodex super gemini --cli agy\n"
            "  prodex super --sub-agent --sub-agent-provider openai --sub-agent-model gpt-5.3-codex\n"
            "  prodex super --sub-agent --sub-agent-model-reasoning-effort xhigh\n"
            "  prodex super --sub-agent --sub-agent-max-concurrency 8\n"
            "  prodex super --no-sub-agent\n"
            "  prodex super exec \"review latest diff in super mode\"\n"
            "  prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9\n"
            "  prodex super --profile main\n"
            "  prodex super --web-search indexed\n"
            "  prodex super --rollout-budget-tokens 100000\n\n"
            "Notes:\n"
            "  `prodex super` enables Smart Context and the available typed optimizer tools in the temporary Prodex overlay.\n"
            "  Missing optional tools are skipped unless named with `--require-tool`; use `prodex doctor --install` to inspect local prerequisites.\n"
            "  Super is the explicit YOLO path: it bypasses approvals and the sandbox, bypasses hook-trust confirmation, and trusts the current workspace for this invocation.\n"
            "  Codex runtime feature overrides from `prodex run` also work here; explicit `--web-search` overrides Super provider defaults.\n"
            "  Interactive Super launches ask whether to enable Presidio. Use `--presidio` or `--no-presidio` to answer non-interactively. With default endpoints, Presidio opt-in best-effort starts local Docker services unless PRODEX_PRESIDIO_AUTO_START=0.\n"
            "  Use `prodex run` instead when normal Codex approvals and workspace-trust prompts are desired.\n"
            "  Use `prodex doctor --install` to inspect local runtime prerequisites without launching Codex.\n"
            "  Use `--url` to point Codex directly at a local OpenAI-compatible /v1 endpoint, for example a llama-server on port 8131.\n"
            "  When `--url` is set, Prodex injects a temporary `prodex-local` model provider, skips quota/rotation, and uses a local Smart Context rewrite proxy.\n"
            "  Use `--provider anthropic` to route through Anthropic's OpenAI-compatible Chat Completions API. Sign in with `prodex login --with-claude`, or supply `--api-key`, ANTHROPIC_API_KEY, or ANTHROPIC_API_KEYS.\n"
            "  Use `--provider copilot` to keep Codex/Super and route through a local Responses-to-Copilot adapter. Import Copilot profiles first for account routing/rotation, or supply `--api-key`, GITHUB_COPILOT_API_KEY, or GITHUB_COPILOT_API_KEYS.\n"
            "  Use `--provider copilot` for the GitHub Copilot provider bridge.\n"
            "  Use `deepseek` or `--provider deepseek` to keep Codex/Super and route through a local Responses-to-DeepSeek adapter. Supply `--api-key`, DEEPSEEK_API_KEY, or DEEPSEEK_API_KEYS.\n"
            "  Use `gemini` or `--provider gemini` to route Codex through Gemini with `--api-key`, GEMINI_API_KEY, GEMINI_API_KEYS, GOOGLE_API_KEY, or GOOGLE_API_KEYS.\n"
            "  Google Gemini OAuth profile routing is unsupported and disabled. Existing OAuth profiles remain readable for migration diagnostics but cannot launch.\n"
            "  Use `--provider gemini` or positional `gemini` with a Gemini API key for the Gemini provider bridge.\n"
            "  Use `--provider kiro --profile <name>` with an imported Kiro profile for the application-level Codex-to-Kiro ACP bridge.\n"
            "  Add `--cli agy` to launch Antigravity CLI with `--dangerously-skip-permissions`. Antigravity owns its keyring auth and currently cannot use Prodex account rotation. Override the binary with PRODEX_AGY_BIN.\n"
            "  Local mode defaults to a 16k context window; use `--context-window` and `--auto-compact-token-limit` if your server is configured larger.\n"
            "  --sub-agent explicitly enables sub-agents; --no-sub-agent explicitly disables them.\n"
            "  --sub-agent-provider, --sub-agent-model, --sub-agent-model-reasoning-effort, --sub-agent-url, and --sub-agent-max-concurrency require --sub-agent.\n"
            "  Sub-agent provider names use canonical ProviderId values and default to openai; model ids are arbitrary nonempty strings.\n"
            "  Sub-agent reasoning efforts are none, minimal, low, medium, high, xhigh, max, or ultra.\n"
            "  Maximum active sub-agents defaults to 4; presets are 4, 8, 16, and 32, with custom values from 1 through 64.\n"
            "  Additional Codex args are appended unchanged after Prodex's generated options.\n"
            "  The expose URL is an ephemeral full-Super bearer capability. Anyone with the complete URL can control the configured workspace session; this is not OAuth or multi-user authentication. Use `--no-tunnel` or select Local only for loopback access."
        ))
    if kind == CLI_HELP_DOCTOR:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex doctor\n"
            "  prodex doctor --install\n"
            "  prodex doctor --quota\n"
            "  prodex doctor --runtime\n"
            "  prodex doctor --runtime --json\n"
            "  prodex doctor --bundle ./prodex-doctor.json --redacted\n"
            "  prodex doctor --repair-session-index"
        ))
    if kind == CLI_HELP_SESSION:
        return cli_literal(StringSlice(
            "Examples:\n"
            "  prodex session list\n"
            "  prodex session list --json\n"
            "  prodex session list --id-only\n"
            "  prodex session list --resume-command\n"
            "  prodex session list --parent-only\n"
            "  prodex session list --profile main --query triage\n"
            "  prodex session current\n"
            "  prodex session current --resume-command\n"
            "  prodex session current --limit 20\n"
            "  prodex session resume 1234abcd"
        ))
    return ProdexRichStringView(0, 0)


def cli_command_label(kind: Int64) -> ProdexRichStringView:
    if kind == CLI_COMMAND_PROFILE:
        return cli_literal(StringSlice("profile"))
    if kind == CLI_COMMAND_USE_PROFILE:
        return cli_literal(StringSlice("use"))
    if kind == CLI_COMMAND_CURRENT:
        return cli_literal(StringSlice("current"))
    if kind == CLI_COMMAND_INFO:
        return cli_literal(StringSlice("info"))
    if kind == CLI_COMMAND_STATUS:
        return cli_literal(StringSlice("status"))
    if kind == CLI_COMMAND_LOG:
        return cli_literal(StringSlice("log"))
    if kind == CLI_COMMAND_SESSION:
        return cli_literal(StringSlice("session"))
    if kind == CLI_COMMAND_DOCTOR:
        return cli_literal(StringSlice("doctor"))
    if kind == CLI_COMMAND_LOGIN:
        return cli_literal(StringSlice("login"))
    if kind == CLI_COMMAND_LOGOUT:
        return cli_literal(StringSlice("logout"))
    if kind == CLI_COMMAND_UPDATE:
        return cli_literal(StringSlice("update"))
    if kind == CLI_COMMAND_QUOTA:
        return cli_literal(StringSlice("quota"))
    if kind == CLI_COMMAND_REDEEM:
        return cli_literal(StringSlice("redeem"))
    if kind == CLI_COMMAND_PING:
        return cli_literal(StringSlice("ping"))
    if kind == CLI_COMMAND_RUN:
        return cli_literal(StringSlice("run"))
    if kind == CLI_COMMAND_SUPER:
        return cli_literal(StringSlice("super"))
    if kind == CLI_COMMAND_GATEWAY:
        return cli_literal(StringSlice("gateway"))
    if kind == CLI_COMMAND_SUPER_EXPOSE:
        return cli_literal(StringSlice("super-expose"))
    if kind == CLI_COMMAND_RUNTIME_BROKER:
        return cli_literal(StringSlice("__runtime-broker"))
    if kind == CLI_COMMAND_MCP_JSONL_BRIDGE:
        return cli_literal(StringSlice("__mcp-jsonl-bridge"))
    if kind == CLI_COMMAND_SUB_AGENT_EXEC:
        return cli_literal(StringSlice("__sub-agent-exec"))
    return ProdexRichStringView(0, 0)


@export("prodex_cli_command_runtime_v1")
def prodex_cli_command_runtime_v1(abi: Int64, kind: Int64) abi("C") -> Int64:
    if abi != CLI_PRESENTATION_ABI:
        return -2
    if kind < 0 or kind >= CLI_COMMAND_COUNT:
        return -1
    return Int64(
        kind == CLI_COMMAND_RUN
        or kind == CLI_COMMAND_SUPER
        or kind == CLI_COMMAND_GATEWAY
        or kind == CLI_COMMAND_SUPER_EXPOSE
        or kind == CLI_COMMAND_RUNTIME_BROKER
    )


@export("prodex_cli_presentation_text_v1")
def prodex_cli_presentation_text_v1(
    abi: Int64, operation: Int64, kind: Int64, measuring: Int64,
    output_address: UInt64, capacity: Int64, written_address: UInt64,
) abi("C") -> Int64:
    if abi != CLI_PRESENTATION_ABI:
        return 4
    if (
        operation < CLI_PRESENTATION_HELP or operation > CLI_PRESENTATION_LABEL
        or kind < 0 or measuring < 0 or measuring > 1 or capacity < 0
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    if (operation == CLI_PRESENTATION_HELP and kind >= CLI_HELP_COUNT) or (operation == CLI_PRESENTATION_LABEL and kind >= CLI_COMMAND_COUNT):
        return 1
    if measuring == 1:
        if output_address != 0 or capacity != 0:
            return 1
    elif output_address == 0:
        return 1
    var text = cli_help_text(kind) if operation == CLI_PRESENTATION_HELP else cli_command_label(kind)
    var length = Int64(text.len)
    if measuring == 0:
        if capacity < length:
            return 2
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
        var source = rich_view_ptr(text)
        for index in range(length):
            output[unsafe_offset=index] = source[unsafe_offset=index]
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = length
    return 0
