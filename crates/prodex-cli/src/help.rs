pub const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
pub const CLI_TOP_LEVEL_AFTER_HELP: &str = "\
Tips:
  Bare `prodex` invocation defaults to `prodex run`.
  Use `prodex status` for the live quota, token, and resource dashboard.
  Use `prodex quota --all --detail` for the clearest quota view across profiles.
  Use `prodex <command> -h` to see every parameter for that command.

Examples:
  prodex
  prodex exec \"review this repo\"
  prodex profile list
  prodex status
  prodex quota --all --detail
  prodex redeem main
  prodex run --profile main";
pub const CLI_PROFILE_AFTER_HELP: &str = "\
Examples:
  prodex profile list
  prodex profile add main --activate
  prodex profile add main --insecure
  prodex profile export
  prodex profile export backup.json
  prodex profile import backup.json
  prodex profile import backup.json --insecure
  prodex profile import claude
  prodex profile import copilot
  prodex profile import kiro
  prodex profile import copilot --name copilot-main --activate
  prodex profile import-current main
  prodex profile remove main
  prodex profile remove --all

Notes:
  `--insecure` bypasses local private-file permission and ACL validation; use it only with trusted files.
  `prodex profile import kiro` reads the installed Kiro CLI auth store and profile metadata.
  Prodex prefers `kiro-cli-chat`, falls back to `kiro-cli`, and accepts `PRODEX_KIRO_BIN` for an explicit path override.";
pub const CLI_LOGIN_AFTER_HELP: &str = "\
Examples:
  prodex login
  prodex login main
  prodex login --profile main
  prodex login --device-auth
  prodex login --with-claude
  prodex login --with-antigravity

Notes:
  A leading non-option argument selects the profile; `status` remains Codex login status.
  Use --profile when selecting a profile literally named `status`.
  OpenAI/Codex, Claude, and API-key login paths create or update Prodex profiles.
  Google Gemini OAuth is unsupported. Use a Gemini API key for the Codex bridge, or native `prodex s gemini --cli gemini` for supported Vertex AI authentication.
  Antigravity login delegates to `agy auth login` and does not create a Prodex profile.";
pub const CLI_QUOTA_AFTER_HELP: &str = "\
Best practice:
  Use `prodex quota --all --detail` for the clearest live quota view across profiles.

Examples:
  prodex quota
  prodex quota --profile main --detail
  prodex quota --all --detail
  prodex quota --all --detail --provider openai
  prodex quota --all --provider deepseek --base-url https://api.deepseek.com --once
  prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once
  prodex quota --all --once
  prodex quota --all --auth no-auth --once
  prodex quota --raw --profile main

Notes:
  `prodex quota` supports OpenAI/Codex, Anthropic OAuth, imported Copilot, DeepSeek API-key, Antigravity CLI, local OpenAI-compatible, and custom provider snapshots.
  Use `--provider` with `--all` to filter by provider: `openai`, `gemini`, `anthropic`, `claude`, `copilot`, `kiro`, `deepseek`, `local`, or `agy`.
  Use `--auth` with `--all` to filter by auth label or compatibility, for example `no-auth` or `quota-compatible`.
  If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock` or `amazon-bedrock-runtime`, prodex shows a provider snapshot instead of failing the quota view.";
pub const CLI_REDEEM_AFTER_HELP: &str = "\
Examples:
  prodex redeem main
  prodex redeem user_example.com
  prodex redeem main --yes
  prodex redeem main --base-url https://chatgpt.com/backend-api

Notes:
  Manual redeem sends one explicit reset-credit consume request for the named OpenAI/Codex profile.
  If the 5h or weekly quota window resets within 1 hour, prodex asks before consuming a credit; pass `--yes` to skip that prompt.
  The backend decides whether a credit can be applied and may return nothing-to-reset, no-credit, or already-redeemed.";
pub const CLI_RUN_AFTER_HELP: &str = "\
Examples:
  prodex
  prodex run
  prodex super
  prodex exec \"review this repo\"
  prodex run --profile main
  prodex run --web-search indexed
  prodex run --rollout-budget-tokens 100000
  prodex run --current-time-reminder
  prodex run --respect-system-proxy
  prodex run exec \"review this repo\"
  prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Eligible pre-commit rotation is allowed by default when another supported profile or key is available.
  Bare `prodex <args>` is treated as `prodex run <args>`.
  A lone session id is forwarded as `codex resume <session-id>`.
  Codex runtime feature overrides are passed through with `-c`: `--web-search disabled|cached|indexed|live`, `--rollout-budget-tokens`, `--current-time-reminder`, and `--respect-system-proxy` / `--no-respect-system-proxy`.
  If the selected profile's `config.toml` sets `model_provider` to a non-OpenAI backend, prodex launches Codex directly without quota preflight or the local auto-rotate proxy.";
pub const CLI_CLAUDE_AFTER_HELP: &str = "\
Examples:
  prodex claude --print \"summarize this repo\"
  prodex claude caveman
  prodex claude caveman -- -p \"summarize this repo briefly\"
  prodex claude --profile main --print \"review the latest changes\"
  prodex claude --skip-quota-check -- --help

Notes:
  Prodex injects a local Anthropic-compatible proxy via `ANTHROPIC_BASE_URL`.
  Prefix Claude args with `caveman` to load the Caveman plugin for that session only.
  Use `PRODEX_CLAUDE_BIN` to point prodex at a specific Claude Code binary.
  `prodex claude` requires the default OpenAI/Codex provider; profiles that set `model_provider` to a non-OpenAI backend are not supported on this path.
  Claude defaults to the current Codex model from `config.toml` when available.
  Use `PRODEX_CLAUDE_MODEL` to override the upstream Responses model mapping.
  Use `PRODEX_CLAUDE_REASONING_EFFORT` to force the upstream Responses reasoning effort.
  Use `PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS=shell,computer` to opt into native client-tool translation on supported models.";
pub const CLI_CAVEMAN_AFTER_HELP: &str = "\
Examples:
  prodex caveman
  prodex rtk
  prodex playwright
  prodex ponytail
  prodex super
  prodex caveman --profile main
  prodex caveman --web-search cached
  prodex caveman exec \"review latest diff in caveman mode\"
  prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Prodex launches Codex from a temporary Prodex overlay `CODEX_HOME`, then activates Caveman for that session.
  The selected profile's auth, shared sessions, and quota behavior stay the same as `prodex run`.
  Codex runtime feature overrides from `prodex run` also work here.
  If the selected profile's `config.toml` sets `model_provider` to a non-OpenAI backend, prodex launches Caveman directly without quota preflight or the local auto-rotate proxy.
  Add prefixes before Codex args to enable session tools in the Prodex overlay: `rtk`, `playwright`, `ponytail`, `presidio`.
  Top-level shortcuts `prodex rtk`, `prodex playwright`, and `prodex ponytail` map to `prodex caveman <prefix>`.
  Caveman must be installed externally as a validated, versioned optional tool; Prodex never downloads it during launch.";
pub const CLI_SUPER_AFTER_HELP: &str = "\
Examples:
  prodex super
  prodex super --model gpt-5.3-codex
  prodex super --url http://127.0.0.1:8131
  prodex super deepseek --model deepseek-v4-pro
  prodex super gemini
  prodex super gemini --cli gemini
  prodex super --provider copilot --cli copilot
  prodex super --cli kiro --profile kiro-main
  prodex super gemini --cli agy
  prodex super --sub-agent --sub-agent-provider openai --sub-agent-model gpt-5.3-codex
  prodex super --sub-agent --sub-agent-model-reasoning-effort xhigh
  prodex super --sub-agent --sub-agent-max-concurrency 8
  prodex super --no-sub-agent
  prodex super doctor
  prodex super doctor --json --strict
  prodex super exec \"review latest diff in super mode\"
  prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
  prodex super --profile main
  prodex super --web-search indexed
  prodex super --rollout-budget-tokens 100000

Notes:
  `prodex super` enables Smart Context and the available typed optimizer tools in the temporary Prodex overlay.
  Missing optional tools are skipped unless named with `--require-tool`; use `prodex super doctor` to inspect readiness.
  Super is the explicit YOLO path: it bypasses approvals and the sandbox, bypasses hook-trust confirmation, and trusts the current workspace for this invocation.
  Codex runtime feature overrides from `prodex run` also work here; explicit `--web-search` overrides Super provider defaults.
  Interactive Super launches ask whether to enable Presidio. Use `--presidio` or `--no-presidio` to answer non-interactively. With default endpoints, Presidio opt-in best-effort starts local Docker services unless PRODEX_PRESIDIO_AUTO_START=0.
  Use `prodex run` instead when normal Codex approvals and workspace-trust prompts are desired.
  Use `prodex super doctor` or `prodex s doctor` to inspect local optimizer readiness without launching Codex.
  Use `--url` to point Codex directly at a local OpenAI-compatible /v1 endpoint, for example a llama-server on port 8131.
  When `--url` is set, Prodex injects a temporary `prodex-local` model provider, skips quota/rotation, and uses a local Smart Context rewrite proxy.
  Use `--provider anthropic` to route through Anthropic's OpenAI-compatible Chat Completions API. Sign in with `prodex login --with-claude`, or supply `--api-key`, ANTHROPIC_API_KEY, or ANTHROPIC_API_KEYS.
  Use `--provider copilot` to keep Codex/Super and route through a local Responses-to-Copilot adapter. Import Copilot profiles first for account routing/rotation, or supply `--api-key`, GITHUB_COPILOT_API_KEY, or GITHUB_COPILOT_API_KEYS.
  Add `--cli copilot` with `--provider copilot` to launch GitHub Copilot CLI through the same Prodex Responses proxy. Override the binary with PRODEX_COPILOT_BIN.
  Use `deepseek` or `--provider deepseek` to keep Codex/Super and route through a local Responses-to-DeepSeek adapter. Supply `--api-key`, DEEPSEEK_API_KEY, or DEEPSEEK_API_KEYS.
  Use `gemini` or `--provider gemini` to route Codex through Gemini with `--api-key`, GEMINI_API_KEY, GEMINI_API_KEYS, GOOGLE_API_KEY, or GOOGLE_API_KEYS.
  Google Gemini OAuth profile routing is unsupported and disabled. Existing OAuth profiles remain readable for migration diagnostics but cannot launch.
  Add `--cli gemini` to launch the native Gemini CLI with authentication, including supported Vertex AI configuration, owned by that CLI or its environment. Prodex does not inject or reuse the removed OAuth client. Override the binary with PRODEX_GEMINI_BIN.
  Add `--cli kiro` to launch Kiro CLI from an imported Kiro profile snapshot through an authenticated loopback CONNECT tunnel. Kiro's TLS payload stays opaque; `--provider kiro` is the application-level Codex-to-Kiro ACP bridge. Override the binary with PRODEX_KIRO_BIN.
  Add `--cli agy` to launch Antigravity CLI with `--dangerously-skip-permissions`. Antigravity owns its keyring auth and currently cannot use Prodex account rotation. Override the binary with PRODEX_AGY_BIN.
  Local mode defaults to a 16k context window; use `--context-window` and `--auto-compact-token-limit` if your server is configured larger.
  --sub-agent explicitly enables sub-agents; --no-sub-agent explicitly disables them.
  --sub-agent-provider, --sub-agent-model, --sub-agent-model-reasoning-effort, --sub-agent-url, and --sub-agent-max-concurrency require --sub-agent.
  Sub-agent provider names use canonical ProviderId values and default to openai; model ids are arbitrary nonempty strings.
  Sub-agent reasoning efforts are none, minimal, low, medium, high, xhigh, or max.
  Maximum active sub-agents defaults to 4; presets are 4, 8, 16, and 32, with custom values from 1 through 64.
  Additional Codex args are appended unchanged after Prodex's generated options.";
pub const CLI_DOCTOR_AFTER_HELP: &str = "\
Examples:
  prodex doctor
  prodex doctor --install
  prodex doctor --quota
  prodex doctor --runtime
  prodex doctor --runtime --json
  prodex doctor --bundle ./prodex-doctor.json --redacted
  prodex doctor --repair-session-index";
pub const CLI_SETUP_AFTER_HELP: &str = "\
Examples:
  prodex setup --dry-run
  prodex setup --verify-tools
  prodex setup --dry-run --json

`--verify-tools` checks Caveman, RTK, Codebase Memory MCP, Playwright MCP, and Ponytail.";
pub const CLI_CAPABILITY_AFTER_HELP: &str = "\
Examples:
  prodex capability list
  prodex capability list --json";
pub const CLI_AUDIT_AFTER_HELP: &str = "\
Examples:
  prodex audit
  prodex audit --tail 50
  prodex audit --component profile --action use
  prodex audit --json";
pub const CLI_CONTEXT_AFTER_HELP: &str = "\
Examples:
  prodex context audit
  prodex context audit --limit 30
  prodex context audit --json
  prodex context export 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
  prodex context export 019c9e3d ./context_session.md
  prodex context compress ~/.codex/AGENTS.md --dry-run
  prodex context compress ~/.codex/AGENTS.md
  prodex context replay-report smart-context-replay.json
  prodex context replay-report smart-context-replay.json --json
  prodex context replay-report smart-context-replay.json --strict";
pub const CLI_SESSION_AFTER_HELP: &str = "\
Examples:
  prodex session list
  prodex session list --json
  prodex session list --id-only
  prodex session list --resume-command
  prodex session list --parent-only
  prodex session list --profile main --query triage
  prodex session current
  prodex session current --resume-command
  prodex session current --limit 20
  prodex session resume 1234abcd";
pub const CLI_CLEANUP_AFTER_HELP: &str = "\
Examples:
  prodex cleanup
  prodex cleanup --older-than 1d
  prodex cleanup --aggressive

Notes:
  Removes stale local artifacts. Non-blocking automatic housekeeping already clears stale runtime logs, temp homes, stale root temp files, and dead broker artifacts. --older-than controls only orphaned managed profile homes; --aggressive is equivalent to --older-than 0d. Codex/Claude chat histories are left to the upstream runtimes.";
