use codex_config::codex_config_value;
use prodex_mojo_core::codex_auth_headers::{CodexTerminalInput, CodexTerminalKind as TerminalName};
use prodex_mojo_core::rich::ascii_casefold_equal_exact;
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
            ascii_casefold_equal_exact(value.trim(), CODEX_OPENAI_AUTH_RESIDENCY_US)
                .expect("Mojo OpenAI residency comparison failed")
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
    let build_version = codex_cli_version()?;
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

pub(crate) fn codex_cli_version() -> Option<&'static str> {
    static CODEX_VERSION: OnceLock<Option<String>> = OnceLock::new();
    CODEX_VERSION
        .get_or_init(resolve_codex_cli_version)
        .as_deref()
}

fn resolve_codex_cli_version() -> Option<String> {
    let mut command = Command::new(crate::codex_bin());
    command
        .arg("--version")
        .env_remove("CODEX_HOME")
        .env_remove("TEST_CODEX_LOG")
        .env_remove("TEST_CODEX_LOG_APPEND")
        .env_remove("TEST_CODEX_ARGS_LOG")
        .env_remove("TEST_CODEX_ARGS_LOG_APPEND")
        .env_remove("TEST_CODEX_STDIN_LOG");
    let output = crate::command_probe_output(&mut command, "Codex version probe").ok()?;
    if !output.status.success() {
        return None;
    }
    parse_codex_cli_version_output(&String::from_utf8(output.stdout).ok()?)
}

pub(crate) fn parse_codex_cli_version_output(output: &str) -> Option<String> {
    prodex_mojo_core::codex_auth_headers::codex_probe_version(output)
        .expect("compiled Mojo Codex version-probe lexer returned invalid output")
        .map(ToOwned::to_owned)
}

fn sanitize_codex_user_agent(candidate: String, fallback: &str) -> String {
    if reqwest::header::HeaderValue::from_str(candidate.as_str()).is_ok() {
        return candidate;
    }

    let sanitized = prodex_mojo_core::codex_auth_headers::codex_user_agent_printable(&candidate)
        .expect("compiled Mojo Codex User-Agent sanitizer returned invalid output");
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
        .get_or_init(detect_terminal_info)
        .user_agent_token()
}

#[derive(Clone, Debug)]
struct TerminalInfo {
    name: TerminalName,
    term_program: Option<String>,
    version: Option<String>,
    term: Option<String>,
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
        let kind = prodex_mojo_core::codex_auth_headers::codex_terminal_kind(
            &term,
            CodexTerminalInput::Term,
        )
        .expect("compiled Mojo exact TERM fallback classifier failed");
        Self::new(kind, None, None, Some(term))
    }

    fn unknown() -> Self {
        Self::new(TerminalName::Unknown, None, None, None)
    }

    fn user_agent_token(&self) -> String {
        prodex_mojo_core::codex_auth_headers::codex_terminal_token(
            self.name,
            self.term_program.as_deref(),
            self.version.as_deref(),
            self.term.as_deref(),
        )
        .expect("compiled Mojo terminal User-Agent projection returned invalid output")
    }
}

fn detect_terminal_info() -> TerminalInfo {
    let tmux = terminal_env_has_non_empty("TMUX") || terminal_env_has_non_empty("TMUX_PANE");
    if let Some(terminal) = detect_terminal_from_term_program(tmux) {
        return terminal;
    }
    detect_terminal_from_environment().unwrap_or_else(TerminalInfo::unknown)
}

fn detect_terminal_from_term_program(tmux: bool) -> Option<TerminalInfo> {
    let term_program = terminal_env_var_non_empty("TERM_PROGRAM")?;
    if ascii_casefold_equal_exact(&term_program, "tmux")
        .expect("Mojo terminal-program comparison failed")
        && tmux
        && let Some(terminal) = terminal_from_tmux_client_info(tmux_client_info())
    {
        return Some(terminal);
    }
    let version = terminal_env_var_non_empty("TERM_PROGRAM_VERSION");
    let name = terminal_name_from_term_program(&term_program);
    Some(TerminalInfo::from_term_program(name, term_program, version))
}

fn detect_terminal_from_environment() -> Option<TerminalInfo> {
    if terminal_env_has("WEZTERM_VERSION") {
        return Some(TerminalInfo::from_name(
            TerminalName::WezTerm,
            terminal_env_var_non_empty("WEZTERM_VERSION"),
        ));
    }
    if terminal_env_has("ITERM_SESSION_ID")
        || terminal_env_has("ITERM_PROFILE")
        || terminal_env_has("ITERM_PROFILE_NAME")
    {
        return Some(TerminalInfo::from_name(TerminalName::Iterm2, None));
    }
    if terminal_env_has("TERM_SESSION_ID") {
        return Some(TerminalInfo::from_name(TerminalName::AppleTerminal, None));
    }
    if terminal_env_has("KITTY_WINDOW_ID")
        || terminal_env_var("TERM").is_some_and(|term| term.contains("kitty"))
    {
        return Some(TerminalInfo::from_name(TerminalName::Kitty, None));
    }
    if terminal_env_has("ALACRITTY_SOCKET")
        || terminal_env_var("TERM").is_some_and(|term| term == "alacritty")
    {
        return Some(TerminalInfo::from_name(TerminalName::Alacritty, None));
    }
    if terminal_env_has("KONSOLE_VERSION") {
        return Some(TerminalInfo::from_name(
            TerminalName::Konsole,
            terminal_env_var_non_empty("KONSOLE_VERSION"),
        ));
    }
    if terminal_env_has("GNOME_TERMINAL_SCREEN") {
        return Some(TerminalInfo::from_name(TerminalName::GnomeTerminal, None));
    }
    if terminal_env_has("VTE_VERSION") {
        return Some(TerminalInfo::from_name(
            TerminalName::Vte,
            terminal_env_var_non_empty("VTE_VERSION"),
        ));
    }
    if terminal_env_has("WT_SESSION") {
        return Some(TerminalInfo::from_name(TerminalName::WindowsTerminal, None));
    }
    if let Some(term) = terminal_env_var_non_empty("TERM") {
        return Some(TerminalInfo::from_term(term));
    }
    None
}

fn terminal_env_var(name: &str) -> Option<String> {
    env::var(name).ok()
}

fn terminal_env_has(name: &str) -> bool {
    terminal_env_var(name).is_some()
}

fn terminal_env_var_non_empty(name: &str) -> Option<String> {
    terminal_env_var(name).and_then(none_if_whitespace)
}

fn terminal_env_has_non_empty(name: &str) -> bool {
    terminal_env_var_non_empty(name).is_some()
}

fn tmux_client_info() -> TmuxClientInfo {
    TmuxClientInfo {
        termtype: tmux_display_message("#{client_termtype}"),
        termname: tmux_display_message("#{client_termname}"),
    }
}

fn terminal_from_tmux_client_info(client_info: TmuxClientInfo) -> Option<TerminalInfo> {
    let termtype = client_info.termtype.and_then(none_if_whitespace);
    let termname = client_info.termname.and_then(none_if_whitespace);

    if let Some(termtype) = termtype {
        let (program, version) = split_term_program_and_version(&termtype);
        let name = terminal_name_from_term_program(&program);
        return Some(TerminalInfo::new(name, Some(program), version, termname));
    }

    termname.map(TerminalInfo::from_term)
}

fn split_term_program_and_version(value: &str) -> (String, Option<String>) {
    prodex_mojo_core::codex_auth_headers::codex_term_program_parts(value)
        .expect("compiled Mojo tmux TERM_PROGRAM lexer returned invalid output")
}

fn tmux_display_message(format: &str) -> Option<String> {
    let mut command = Command::new("tmux");
    command.args(["display-message", "-p", format]);
    let output = crate::command_probe_output(&mut command, "tmux client probe").ok()?;
    if !output.status.success() {
        return None;
    }
    none_if_whitespace(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

fn terminal_name_from_term_program(value: &str) -> TerminalName {
    prodex_mojo_core::codex_auth_headers::codex_terminal_kind(
        value,
        CodexTerminalInput::TermProgram,
    )
    .expect("compiled Mojo TERM_PROGRAM classifier returned invalid output")
}

fn none_if_whitespace(value: String) -> Option<String> {
    let nonblank = prodex_mojo_core::codex_auth_headers::codex_nonwhitespace(&value)
        .expect("compiled Mojo terminal environment whitespace policy failed");
    nonblank.then_some(value)
}

#[cfg(test)]
mod compiled_auth_headers_tests {
    use super::*;

    #[test]
    fn compiled_codex_terminal_token_uses_original_metadata_and_safe_scalars() {
        let terminal = TerminalInfo::new(
            TerminalName::Ghostty,
            Some("Custom 🦀 Terminal".into()),
            Some("2.0\nβ".into()),
            Some("ignored-term".into()),
        );
        assert_eq!(terminal.user_agent_token(), "Custom___Terminal/2.0__");
        assert_eq!(
            TerminalInfo::from_term("xterm-256color".into()).user_agent_token(),
            "xterm-256color"
        );
        assert_eq!(
            TerminalInfo::from_name(TerminalName::Kitty, Some("ignored".into())).user_agent_token(),
            "kitty"
        );
        assert_eq!(TerminalInfo::unknown().user_agent_token(), "unknown");
    }

    #[test]
    fn compiled_codex_probe_preserves_unicode_whitespace_and_last_candidate() {
        assert_eq!(
            parse_codex_cli_version_output("codex-cli 0.100.1\u{2003}2.3.4!"),
            Some("2.3.4".to_owned())
        );
        assert_eq!(parse_codex_cli_version_output("codex-cli v0.100.1"), None);
    }

    #[test]
    fn compiled_terminal_header_token_replaces_invalid_scalars_and_preserves_value() {
        let terminal = TerminalInfo::from_term_program(
            TerminalName::Ghostty,
            "Ghostty".to_owned(),
            Some("1.2 🌍".to_owned()),
        );
        assert_eq!(terminal.user_agent_token(), "Ghostty/1.2__");
        assert_eq!(none_if_whitespace(" \u{2003}\u{00a0} ".to_owned()), None);
        assert_eq!(
            none_if_whitespace(" \u{2003}Ghostty ".to_owned()),
            Some(" \u{2003}Ghostty ".to_owned())
        );
        assert_eq!(
            prodex_mojo_core::codex_auth_headers::codex_terminal_header_token("A/B-.\tα").unwrap(),
            "A/B-.__"
        );
    }

    #[test]
    fn compiled_codex_terminal_detection_preserves_tmux_and_program_aliases() {
        assert!(matches!(
            terminal_name_from_term_program("  iTerm.app  "),
            TerminalName::Iterm2
        ));
        assert!(matches!(
            terminal_name_from_term_program("Warp Terminal"),
            TerminalName::WarpTerminal
        ));
        assert_eq!(
            terminal_name_from_term_program("unknown"),
            TerminalName::Unknown
        );
        assert_eq!(
            split_term_program_and_version("\u{2003}Ghostty 1.2.3 extra"),
            ("Ghostty".to_owned(), Some("1.2.3".to_owned()))
        );
        let term = TerminalInfo::from_term("wezterm-mux".to_string());
        assert!(matches!(term.name, TerminalName::WezTerm));
        let term = TerminalInfo::from_term("DUMB".to_string());
        assert!(matches!(term.name, TerminalName::Unknown));
        let terminal = terminal_from_tmux_client_info(TmuxClientInfo {
            termtype: Some("iTerm.app\u{2003}3.5.0 extra".into()),
            termname: Some("xterm-256color".into()),
        })
        .unwrap();
        assert_eq!(terminal.name, TerminalName::Iterm2);
        assert_eq!(terminal.user_agent_token(), "iTerm.app/3.5.0");
    }

    #[test]
    fn compiled_codex_user_agent_sanitizes_invalid_unicode_per_scalar() {
        // HeaderValue accepts a valid UTF-8 field; a linefeed forces the
        // printable fallback, which also replaces non-ASCII scalars.
        let bad = "codex_cli_rs/0.142 (OS 🚀 β)\n";
        assert_eq!(
            sanitize_codex_user_agent(bad.to_owned(), "fallback"),
            "codex_cli_rs/0.142 (OS _ _)_"
        );
        let direct = "codex_cli_rs/0.142 (OS)";
        assert_eq!(
            sanitize_codex_user_agent(direct.to_owned(), "fallback"),
            direct
        );
    }
}
