use codex_config::codex_config_value;
use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

const CODEX_OPENAI_AUTH_ORIGINATOR: &str = "codex_cli_rs";
const CODEX_OPENAI_AUTH_ORIGINATOR_OVERRIDE_ENV: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
const CODEX_OPENAI_AUTH_RESIDENCY_HEADER: &str = "x-openai-internal-codex-residency";
const CODEX_OPENAI_AUTH_RESIDENCY_US: &str = "us";

pub(super) fn codex_openai_auth_headers(
    request: reqwest::blocking::RequestBuilder,
) -> reqwest::blocking::RequestBuilder {
    let originator = codex_openai_auth_originator();
    let request = request.header("originator", originator.as_str());
    match codex_openai_auth_user_agent(&originator) {
        Some(user_agent) => request.header(reqwest::header::USER_AGENT, user_agent),
        None => request,
    }
}

pub(super) fn codex_openai_auth_headers_for_home(
    request: reqwest::blocking::RequestBuilder,
    codex_home: &Path,
) -> codex_config::CodexConfigResult<reqwest::blocking::RequestBuilder> {
    let request = codex_openai_auth_headers(request);
    Ok(match codex_openai_auth_residency(codex_home)? {
        Some(residency) => request.header(CODEX_OPENAI_AUTH_RESIDENCY_HEADER, residency),
        None => request,
    })
}

fn codex_openai_auth_residency(
    codex_home: &Path,
) -> codex_config::CodexConfigResult<Option<&'static str>> {
    Ok(codex_config_value(codex_home, "enforce_residency")?
        .is_some_and(|value| {
            value
                .trim()
                .eq_ignore_ascii_case(CODEX_OPENAI_AUTH_RESIDENCY_US)
        })
        .then_some(CODEX_OPENAI_AUTH_RESIDENCY_US))
}

pub(super) fn codex_openai_auth_originator() -> String {
    let value = env::var(CODEX_OPENAI_AUTH_ORIGINATOR_OVERRIDE_ENV)
        .unwrap_or_else(|_| CODEX_OPENAI_AUTH_ORIGINATOR.to_string());
    if reqwest::header::HeaderValue::from_str(&value).is_ok() {
        value
    } else {
        CODEX_OPENAI_AUTH_ORIGINATOR.to_string()
    }
}

fn codex_openai_auth_user_agent(originator: &str) -> Option<String> {
    let build_version = codex_openai_auth_codex_version()?;
    Some(codex_openai_auth_user_agent_for_version(
        originator,
        build_version,
        codex_terminal_user_agent(),
    ))
}

pub(super) fn codex_openai_auth_user_agent_for_version(
    originator: &str,
    build_version: &str,
    terminal_user_agent: String,
) -> String {
    let os_info = os_info::get();
    let prefix = format!(
        "{originator}/{build_version} ({} {}; {}) {terminal_user_agent}",
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
    );
    sanitize_codex_user_agent(prefix.clone(), &prefix)
}

fn codex_openai_auth_codex_version() -> Option<&'static str> {
    static CODEX_VERSION: OnceLock<Option<String>> = OnceLock::new();
    CODEX_VERSION
        .get_or_init(resolve_codex_cli_version)
        .as_deref()
}

fn resolve_codex_cli_version() -> Option<String> {
    let output = Command::new(crate::codex_bin())
        .arg("--version")
        .env_remove("CODEX_HOME")
        .env_remove("TEST_CODEX_LOG")
        .env_remove("TEST_CODEX_LOG_APPEND")
        .env_remove("TEST_CODEX_ARGS_LOG")
        .env_remove("TEST_CODEX_ARGS_LOG_APPEND")
        .env_remove("TEST_CODEX_STDIN_LOG")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_codex_cli_version_output(&String::from_utf8(output.stdout).ok()?)
}

pub(super) fn parse_codex_cli_version_output(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .rev()
        .find(|token| {
            token
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
                && token.bytes().any(|byte| byte == b'.')
        })
        .map(|token| {
            token
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-')
                .to_string()
        })
        .filter(|token| !token.is_empty())
}

fn sanitize_codex_user_agent(candidate: String, fallback: &str) -> String {
    if reqwest::header::HeaderValue::from_str(candidate.as_str()).is_ok() {
        return candidate;
    }

    let sanitized = candidate
        .chars()
        .map(|ch| if matches!(ch, ' '..='~') { ch } else { '_' })
        .collect::<String>();
    if !sanitized.is_empty() && reqwest::header::HeaderValue::from_str(sanitized.as_str()).is_ok() {
        sanitized
    } else if reqwest::header::HeaderValue::from_str(fallback).is_ok() {
        fallback.to_string()
    } else {
        CODEX_OPENAI_AUTH_ORIGINATOR.to_string()
    }
}

fn codex_terminal_user_agent() -> String {
    static TERMINAL_INFO: OnceLock<TerminalInfo> = OnceLock::new();
    TERMINAL_INFO
        .get_or_init(|| detect_terminal_info(&ProcessEnvironment))
        .user_agent_token()
}

#[derive(Clone, Debug)]
struct TerminalInfo {
    name: TerminalName,
    term_program: Option<String>,
    version: Option<String>,
    term: Option<String>,
}

#[derive(Clone, Copy, Debug)]
enum TerminalName {
    AppleTerminal,
    Ghostty,
    Iterm2,
    WarpTerminal,
    VsCode,
    WezTerm,
    Kitty,
    Alacritty,
    Konsole,
    GnomeTerminal,
    Vte,
    WindowsTerminal,
    Dumb,
    Unknown,
}

#[derive(Default)]
struct TmuxClientInfo {
    termtype: Option<String>,
    termname: Option<String>,
}

impl TerminalInfo {
    fn new(
        name: TerminalName,
        term_program: Option<String>,
        version: Option<String>,
        term: Option<String>,
    ) -> Self {
        Self {
            name,
            term_program,
            version,
            term,
        }
    }

    fn from_term_program(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
    ) -> Self {
        Self::new(name, Some(term_program), version, None)
    }

    fn from_name(name: TerminalName, version: Option<String>) -> Self {
        Self::new(name, None, version, None)
    }

    fn from_term(term: String) -> Self {
        let name = match term.as_str() {
            "dumb" => TerminalName::Dumb,
            "wezterm" | "wezterm-mux" => TerminalName::WezTerm,
            _ => TerminalName::Unknown,
        };
        Self::new(name, None, None, Some(term))
    }

    fn unknown() -> Self {
        Self::new(TerminalName::Unknown, None, None, None)
    }

    fn user_agent_token(&self) -> String {
        let raw = if let Some(program) = self.term_program.as_ref() {
            match self.version.as_ref().filter(|value| !value.is_empty()) {
                Some(version) => format!("{program}/{version}"),
                None => program.clone(),
            }
        } else if let Some(term) = self.term.as_ref().filter(|value| !value.is_empty()) {
            term.clone()
        } else {
            match self.name {
                TerminalName::AppleTerminal => {
                    format_terminal_version("Apple_Terminal", &self.version)
                }
                TerminalName::Ghostty => format_terminal_version("Ghostty", &self.version),
                TerminalName::Iterm2 => format_terminal_version("iTerm.app", &self.version),
                TerminalName::WarpTerminal => {
                    format_terminal_version("WarpTerminal", &self.version)
                }
                TerminalName::VsCode => format_terminal_version("vscode", &self.version),
                TerminalName::WezTerm => format_terminal_version("WezTerm", &self.version),
                TerminalName::Kitty => "kitty".to_string(),
                TerminalName::Alacritty => "Alacritty".to_string(),
                TerminalName::Konsole => format_terminal_version("Konsole", &self.version),
                TerminalName::GnomeTerminal => "gnome-terminal".to_string(),
                TerminalName::Vte => format_terminal_version("VTE", &self.version),
                TerminalName::WindowsTerminal => "WindowsTerminal".to_string(),
                TerminalName::Dumb => "dumb".to_string(),
                TerminalName::Unknown => "unknown".to_string(),
            }
        };

        sanitize_terminal_header_value(raw)
    }
}

trait TerminalEnvironment {
    fn var(&self, name: &str) -> Option<String>;

    fn has(&self, name: &str) -> bool {
        self.var(name).is_some()
    }

    fn var_non_empty(&self, name: &str) -> Option<String> {
        self.var(name).and_then(none_if_whitespace)
    }

    fn has_non_empty(&self, name: &str) -> bool {
        self.var_non_empty(name).is_some()
    }

    fn tmux_client_info(&self) -> TmuxClientInfo;
}

struct ProcessEnvironment;

impl TerminalEnvironment for ProcessEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }

    fn tmux_client_info(&self) -> TmuxClientInfo {
        TmuxClientInfo {
            termtype: tmux_display_message("#{client_termtype}"),
            termname: tmux_display_message("#{client_termname}"),
        }
    }
}

fn detect_terminal_info(env: &dyn TerminalEnvironment) -> TerminalInfo {
    let tmux = env.has_non_empty("TMUX") || env.has_non_empty("TMUX_PANE");
    if let Some(term_program) = env.var_non_empty("TERM_PROGRAM") {
        if term_program.eq_ignore_ascii_case("tmux")
            && tmux
            && let Some(terminal) = terminal_from_tmux_client_info(env.tmux_client_info())
        {
            return terminal;
        }

        let version = env.var_non_empty("TERM_PROGRAM_VERSION");
        let name = terminal_name_from_term_program(&term_program).unwrap_or(TerminalName::Unknown);
        return TerminalInfo::from_term_program(name, term_program, version);
    }

    if env.has("WEZTERM_VERSION") {
        return TerminalInfo::from_name(
            TerminalName::WezTerm,
            env.var_non_empty("WEZTERM_VERSION"),
        );
    }
    if env.has("ITERM_SESSION_ID") || env.has("ITERM_PROFILE") || env.has("ITERM_PROFILE_NAME") {
        return TerminalInfo::from_name(TerminalName::Iterm2, None);
    }
    if env.has("TERM_SESSION_ID") {
        return TerminalInfo::from_name(TerminalName::AppleTerminal, None);
    }
    if env.has("KITTY_WINDOW_ID") || env.var("TERM").is_some_and(|term| term.contains("kitty")) {
        return TerminalInfo::from_name(TerminalName::Kitty, None);
    }
    if env.has("ALACRITTY_SOCKET") || env.var("TERM").is_some_and(|term| term == "alacritty") {
        return TerminalInfo::from_name(TerminalName::Alacritty, None);
    }
    if env.has("KONSOLE_VERSION") {
        return TerminalInfo::from_name(
            TerminalName::Konsole,
            env.var_non_empty("KONSOLE_VERSION"),
        );
    }
    if env.has("GNOME_TERMINAL_SCREEN") {
        return TerminalInfo::from_name(TerminalName::GnomeTerminal, None);
    }
    if env.has("VTE_VERSION") {
        return TerminalInfo::from_name(TerminalName::Vte, env.var_non_empty("VTE_VERSION"));
    }
    if env.has("WT_SESSION") {
        return TerminalInfo::from_name(TerminalName::WindowsTerminal, None);
    }
    if let Some(term) = env.var_non_empty("TERM") {
        return TerminalInfo::from_term(term);
    }

    TerminalInfo::unknown()
}

fn terminal_from_tmux_client_info(client_info: TmuxClientInfo) -> Option<TerminalInfo> {
    let termtype = client_info.termtype.and_then(none_if_whitespace);
    let termname = client_info.termname.and_then(none_if_whitespace);

    if let Some(termtype) = termtype {
        let (program, version) = split_term_program_and_version(&termtype);
        let name = terminal_name_from_term_program(&program).unwrap_or(TerminalName::Unknown);
        return Some(TerminalInfo::new(name, Some(program), version, termname));
    }

    termname.map(TerminalInfo::from_term)
}

fn split_term_program_and_version(value: &str) -> (String, Option<String>) {
    let mut parts = value.split_whitespace();
    (
        parts.next().unwrap_or_default().to_string(),
        parts.next().map(ToString::to_string),
    )
}

fn tmux_display_message(format: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", format])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    none_if_whitespace(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

fn terminal_name_from_term_program(value: &str) -> Option<TerminalName> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_' | '.'))
        .map(|ch| ch.to_ascii_lowercase())
        .collect::<String>();

    match normalized.as_str() {
        "appleterminal" => Some(TerminalName::AppleTerminal),
        "ghostty" => Some(TerminalName::Ghostty),
        "iterm" | "iterm2" | "itermapp" => Some(TerminalName::Iterm2),
        "warp" | "warpterminal" => Some(TerminalName::WarpTerminal),
        "vscode" => Some(TerminalName::VsCode),
        "wezterm" => Some(TerminalName::WezTerm),
        "kitty" => Some(TerminalName::Kitty),
        "alacritty" => Some(TerminalName::Alacritty),
        "konsole" => Some(TerminalName::Konsole),
        "gnometerminal" => Some(TerminalName::GnomeTerminal),
        "vte" => Some(TerminalName::Vte),
        "windowsterminal" => Some(TerminalName::WindowsTerminal),
        "dumb" => Some(TerminalName::Dumb),
        _ => None,
    }
}

fn format_terminal_version(name: &str, version: &Option<String>) -> String {
    match version.as_ref().filter(|value| !value.is_empty()) {
        Some(version) => format!("{name}/{version}"),
        None => name.to_string(),
    }
}

fn sanitize_terminal_header_value(value: String) -> String {
    value.replace(
        |ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/')),
        "_",
    )
}

fn none_if_whitespace(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
