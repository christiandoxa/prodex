pub const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;
pub const CLI_TOP_LEVEL_AFTER_HELP: &str = "\
Tips:
  Bare `prodex` invocation defaults to `prodex run`.
  Use `prodex quota --all --detail` for the clearest quota view across profiles.
  Use `prodex <command> -h` to see every parameter for that command.

Examples:
  prodex
  prodex exec \"review this repo\"
  prodex profile list
  prodex quota --all --detail
  prodex run --profile main";
pub const CLI_PROFILE_AFTER_HELP: &str = "\
Examples:
  prodex profile list
  prodex profile add main --activate
  prodex profile export
  prodex profile export backup.json
  prodex profile import backup.json
  prodex profile import claude
  prodex profile import copilot
  prodex profile import copilot --name copilot-main --activate
  prodex profile import-current main
  prodex profile remove main
  prodex profile remove --all";
pub const CLI_LOGIN_AFTER_HELP: &str = "\
Examples:
  prodex login
  prodex login --profile main
  prodex login --device-auth
  prodex login --with-google
  prodex login --with-claude";
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
  `prodex quota` supports OpenAI/Codex, Gemini OAuth, Anthropic OAuth, imported Copilot, DeepSeek API-key, local OpenAI-compatible, and custom provider snapshots.
  Use `--provider` with `--all` to filter by provider: `openai`, `gemini`, `anthropic`, `copilot`, `deepseek`, or `local`.
  Use `--auth` with `--all` to filter by auth label or compatibility, for example `no-auth` or `quota-compatible`.
  If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock`, prodex shows a provider snapshot instead of failing the quota view.";
pub const CLI_RUN_AFTER_HELP: &str = "\
Examples:
  prodex
  prodex run
  prodex super
  prodex run mem
  prodex exec \"review this repo\"
  prodex run --profile main
  prodex run exec \"review this repo\"
  prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Auto-rotate is enabled by default.
  Bare `prodex <args>` is treated as `prodex run <args>`.
  A lone session id is forwarded as `codex resume <session-id>`.
  If the selected profile's `config.toml` sets `model_provider` to a non-OpenAI backend, prodex launches Codex directly without quota preflight or the local auto-rotate proxy.";
pub const CLI_CLAUDE_AFTER_HELP: &str = "\
Examples:
  prodex claude --print \"summarize this repo\"
  prodex claude mem
  prodex claude caveman
  prodex claude caveman mem
  prodex claude caveman -- -p \"summarize this repo briefly\"
  prodex claude --profile main --print \"review the latest changes\"
  prodex claude --skip-quota-check -- --help

Notes:
  Prodex injects a local Anthropic-compatible proxy via `ANTHROPIC_BASE_URL`.
  Prefix Claude args with `caveman` and/or `mem` to load the Caveman or Claude-Mem plugin for that session only.
  Use `PRODEX_CLAUDE_BIN` to point prodex at a specific Claude Code binary.
  `prodex claude` requires the default OpenAI/Codex provider; profiles that set `model_provider` to a non-OpenAI backend are not supported on this path.
  Claude defaults to the current Codex model from `config.toml` when available.
  Use `PRODEX_CLAUDE_MODEL` to override the upstream Responses model mapping.
  Use `PRODEX_CLAUDE_REASONING_EFFORT` to force the upstream Responses reasoning effort.
  Use `PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS=shell,computer` to opt into native client-tool translation on supported models.";
pub const CLI_CAVEMAN_AFTER_HELP: &str = "\
Examples:
  prodex caveman
  prodex caveman mem
  prodex rtk
  prodex sqz
  prodex super
  prodex caveman --profile main
  prodex caveman exec \"review latest diff in caveman mode\"
  prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9

Notes:
  Prodex launches Codex from a temporary overlay `CODEX_HOME` so Caveman stays isolated from the base profile.
  The selected profile's auth, shared sessions, and quota behavior stay the same as `prodex run`.
  If the selected profile's `config.toml` sets `model_provider` to a non-OpenAI backend, prodex launches Caveman directly without quota preflight or the local auto-rotate proxy.
  Prefix Codex args with `mem` to point Claude-Mem transcript watching at the selected Prodex session path.
  Add optimizer prefixes before Codex args to enable launch overlays: `mem`, `rtk`, `sqz`, `tokensavior`, `clawcompactor`, `presidio`.
  Top-level shortcuts such as `prodex rtk`, `prodex sqz`, `prodex tokensavior`, and `prodex clawcompactor` map to `prodex caveman <prefix>`.
  Caveman activation is sourced from Julius Brussee's Caveman plugin and a session-start hook adapted for the current Codex hooks schema.";
pub const CLI_SUPER_AFTER_HELP: &str = "\
Examples:
  prodex super
  prodex super --url http://127.0.0.1:8131
  prodex super deepseek --model deepseek-v4-pro
  prodex super gemini
  prodex super exec \"review latest diff in super mode\"
  prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
  prodex super --profile main

Notes:
  `prodex super` is a shortcut for `prodex caveman mem rtk sqz tokensavior clawcompactor --full-access`, with an interactive Presidio opt-in prompt before launch.
  It always enables the Caveman overlay, the Claude-Mem transcript watcher prefix, RTK shell-command guidance, Super optimizer overlay, and launch-time full access.
  Empty input or `n` keeps Presidio disabled; answer `y` to make it equivalent to `prodex caveman mem rtk sqz tokensavior clawcompactor presidio --full-access`.
  Use `--presidio` or `--no-presidio` to make the Presidio choice non-interactive.
  Use `--mem-super-slim` to store prompt summaries/references instead of full prompt bodies in Claude-Mem recall.
  Use `--url` to point Codex directly at a local OpenAI-compatible /v1 endpoint, for example a llama-server on port 8131.
  When `--url` is set, Prodex injects a temporary `prodex-local` model provider, skips quota/rotation, and uses a local Smart Context rewrite proxy.
  Use `--provider anthropic` to route through Anthropic's OpenAI-compatible Chat Completions API. Sign in with `prodex login --with-claude`, or supply `--api-key`, ANTHROPIC_API_KEY, or ANTHROPIC_API_KEYS.
  Use `--provider copilot` to keep Codex/Super and route through a local Responses-to-Copilot adapter. Import Copilot profiles first for account rotation, or supply `--api-key`, GITHUB_COPILOT_API_KEY, or GITHUB_COPILOT_API_KEYS.
  Use `deepseek` or `--provider deepseek` to keep Codex/Super and route through a local Responses-to-DeepSeek adapter. Supply `--api-key`, DEEPSEEK_API_KEY, or DEEPSEEK_API_KEYS.
  Use `gemini` or `--provider gemini` to route through Gemini. Supply `--api-key`, GEMINI_API_KEY, GEMINI_API_KEYS, GOOGLE_API_KEY, or GOOGLE_API_KEYS; or sign in with Google via `prodex login`.
  Local mode defaults to a 16k context window; use `--context-window` and `--auto-compact-token-limit` if your server is configured larger.
  Additional Codex args are appended after the implied optimizer prefixes.";
pub const CLI_DOCTOR_AFTER_HELP: &str = "\
Examples:
  prodex doctor
  prodex doctor --install
  prodex doctor --quota
  prodex doctor --runtime
  prodex doctor --runtime --json
  prodex doctor --bundle ./prodex-doctor.json --redacted";
pub const CLI_SETUP_AFTER_HELP: &str = "\
Examples:
  prodex setup --dry-run
  prodex setup --verify-assets
  prodex setup --dry-run --json";
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
  prodex context compress ~/.codex/AGENTS.md --dry-run
  prodex context compress ~/.codex/AGENTS.md";
pub const CLI_SESSION_AFTER_HELP: &str = "\
Examples:
  prodex session list
  prodex session list --json
  prodex session list --id-only
  prodex session list --resume-command
  prodex session list --include-subagents
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
