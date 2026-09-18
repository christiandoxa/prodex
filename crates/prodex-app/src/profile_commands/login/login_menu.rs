use super::LoginMethod;
use anyhow::{Context, Result};
use std::io::{self, IsTerminal, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoginMenuAction {
    Method(LoginMethod),
    Guidance(LoginGuidanceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoginGuidanceKind {
    GeminiApiKey,
    AnthropicApiKey,
    DeepSeekApiKey,
    CopilotImport,
}

#[derive(Debug, Clone, Copy)]
struct LoginMenuEntry {
    title: &'static str,
    provider: &'static str,
    auth: &'static str,
    usage: &'static str,
    command: &'static str,
    action: LoginMenuAction,
}

pub(super) fn login_prompt_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn login_menu_entries() -> &'static [LoginMenuEntry] {
    const ENTRIES: &[LoginMenuEntry] = &[
        LoginMenuEntry {
            title: "Sign in with ChatGPT (OpenAI OAuth)",
            provider: "OpenAI / Codex",
            auth: "ChatGPT OAuth",
            usage: "Default quota-aware profile pool for prodex run, prodex s, and prodex caveman.",
            command: "prodex login",
            action: LoginMenuAction::Method(LoginMethod::ChatGpt),
        },
        LoginMenuEntry {
            title: "OpenAI device code",
            provider: "OpenAI / Codex",
            auth: "Device-code OAuth",
            usage: "Same OpenAI/Codex profile type, useful on a terminal without a local browser.",
            command: "prodex login --device-auth",
            action: LoginMenuAction::Method(LoginMethod::DeviceCode),
        },
        LoginMenuEntry {
            title: "Provide your own API key (OpenAI/API-compatible)",
            provider: "OpenAI / local / OpenAI-compatible endpoint",
            auth: "API key stored in the selected Prodex profile",
            usage: "Use OpenAI API billing, or provide a compatible base URL for local/custom endpoints.",
            command: "prodex login --with-api-key [--base-url URL]",
            action: LoginMenuAction::Method(LoginMethod::ApiKey),
        },
        LoginMenuEntry {
            title: "Google Gemini API key",
            provider: "Google Gemini",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass GEMINI_API_KEY, GEMINI_API_KEYS, GOOGLE_API_KEY(S), or --api-key when launching Gemini.",
            command: "GEMINI_API_KEY=... prodex s --provider gemini --model gemini-2.5-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey),
        },
        LoginMenuEntry {
            title: "Anthropic Claude OAuth",
            provider: "Anthropic Claude",
            auth: "Claude Code OAuth profile",
            usage: "Reusable Anthropic profile for prodex s --provider anthropic without API-key storage.",
            command: "prodex login --with-claude",
            action: LoginMenuAction::Method(LoginMethod::Claude),
        },
        LoginMenuEntry {
            title: "Google Antigravity CLI",
            provider: "Google Antigravity",
            auth: "Antigravity CLI keyring / Google Sign-In",
            usage: "Authenticate the native agy CLI used by prodex s gemini --cli agy.",
            command: "prodex login --with-antigravity",
            action: LoginMenuAction::Method(LoginMethod::Antigravity),
        },
        LoginMenuEntry {
            title: "Anthropic API key",
            provider: "Anthropic Claude",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass ANTHROPIC_API_KEY(S) or --api-key.",
            command: "ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::AnthropicApiKey),
        },
        LoginMenuEntry {
            title: "DeepSeek API key",
            provider: "DeepSeek",
            auth: "Runtime API key only",
            usage: "DeepSeek has no OAuth login in Prodex; use an API key for the provider adapter.",
            command: "DEEPSEEK_API_KEY=... prodex s --provider deepseek --model deepseek-v4-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey),
        },
        LoginMenuEntry {
            title: "GitHub Copilot import",
            provider: "GitHub Copilot",
            auth: "Existing Copilot CLI account import",
            usage: "Record the Copilot identity from local Copilot CLI state, then launch with --provider copilot.",
            command: "prodex profile import copilot",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::CopilotImport),
        },
    ];
    ENTRIES
}

pub(super) fn prompt_login_menu_action() -> Result<LoginMenuAction> {
    prompt_login_menu_action_numbered()
}

fn prompt_login_menu_action_numbered() -> Result<LoginMenuAction> {
    let entries = login_menu_entries();
    let mut stderr = io::stderr();
    writeln!(stderr, "Choose login method:")?;
    for (index, entry) in entries.iter().enumerate() {
        writeln!(stderr, "  {}. {}", index + 1, entry.title)?;
        writeln!(stderr, "     Provider: {}", entry.provider)?;
        writeln!(stderr, "     Auth: {}", entry.auth)?;
        writeln!(stderr, "     Use: {}", entry.usage)?;
        writeln!(stderr, "     Command: {}", entry.command)?;
    }
    loop {
        write!(stderr, "Select login method [1]: ")?;
        stderr.flush()?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read login method")?;
        let selected = input.trim();
        if selected.is_empty() {
            return Ok(entries[0].action);
        }
        if let Ok(index) = selected.parse::<usize>()
            && (1..=entries.len()).contains(&index)
        {
            return Ok(entries[index - 1].action);
        }
        writeln!(stderr, "Enter 1 through {}.", entries.len())?;
    }
}

pub(super) fn show_login_guidance(kind: LoginGuidanceKind) -> Result<()> {
    let entry = login_menu_entries()
        .iter()
        .find(|entry| entry.action == LoginMenuAction::Guidance(kind))
        .context("login guidance entry is missing")?;
    let mut stderr = io::stderr();
    writeln!(stderr)?;
    writeln!(stderr, "Provider guidance: {}", entry.title)?;
    writeln!(stderr, "  Provider: {}", entry.provider)?;
    writeln!(stderr, "  Auth: {}", entry.auth)?;
    writeln!(stderr, "  Use: {}", entry.usage)?;
    writeln!(stderr, "  Command: {}", entry.command)?;
    writeln!(
        stderr,
        "  Note: this path is selected at runtime, not stored by prodex login."
    )?;
    writeln!(stderr)?;
    write!(
        stderr,
        "Press Enter to return to login methods, or Ctrl-C to exit."
    )?;
    stderr.flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read login guidance acknowledgement")?;
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/src/profile_commands/login_menu.rs"]
mod login_menu_tests;
