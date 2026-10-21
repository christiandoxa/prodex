//! Compiled Codex version, terminal identity, and User-Agent projections.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_codex_auth_probe_version_span_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_codex_auth_terminal_token_ascii_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_codex_auth_nonwhitespace_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_codex_auth_terminal_name_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_codex_auth_terminal_parts_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_codex_auth_terminal_token_v1(
        abi_version: i64,
        kind: i64,
        program_present: i64,
        fields_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_codex_auth_user_agent_ascii_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the rightmost whitespace-delimited version-like token and trim
/// trailing punctuation. The host copies the Mojo-selected source span.
pub fn codex_probe_version(output: &str) -> Result<Option<&str>, MojoError> {
    if output.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut result = [-2_i64; 2];
    status(unsafe {
        prodex_codex_auth_probe_version_span_v1(
            ABI_VERSION,
            output.as_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    if result == [-1, -1] {
        return Ok(None);
    }
    let start = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start >= end {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(
        output.get(start..end).ok_or(MojoError::InvalidOutput)?,
    ))
}

/// Replace every non-printable/non-ASCII scalar with exactly one underscore.
/// Host HTTP HeaderValue validity checks remain Rust-owned.
fn project_codex_auth_ascii(candidate: &str, terminal: bool) -> Result<String, MojoError> {
    if candidate.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![0_u8; candidate.len()];
    let mut written = -1_i64;
    let input_ptr = candidate.as_ptr() as usize as u64;
    let input_len = i64::try_from(candidate.len()).map_err(|_| MojoError::InvalidInput)?;
    let output_ptr = output.as_mut_ptr() as usize as u64;
    let capacity = i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?;
    let written_ptr = (&mut written as *mut i64) as usize as u64;
    status(unsafe {
        if terminal {
            prodex_codex_auth_terminal_token_ascii_v1(
                ABI_VERSION,
                input_ptr,
                input_len,
                output_ptr,
                capacity,
                written_ptr,
            )
        } else {
            prodex_codex_auth_user_agent_ascii_v1(
                ABI_VERSION,
                input_ptr,
                input_len,
                output_ptr,
                capacity,
                written_ptr,
            )
        }
    })?;
    let size = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if size > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(size);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

/// Printable-ASCII User-Agent fallback when a HeaderValue was rejected.
pub fn codex_user_agent_printable(candidate: &str) -> Result<String, MojoError> {
    project_codex_auth_ascii(candidate, false)
}

/// Exact allowed terminal token alphabet, one underscore per invalid Unicode
/// scalar. No host-side string replacement or sanitizer fallback.
pub fn codex_terminal_header_token(candidate: &str) -> Result<String, MojoError> {
    project_codex_auth_ascii(candidate, true)
}

/// Unicode-aware empty/whitespace predicate used by environment/termtype
/// selection. The host retains the original string when this returns true.
pub fn codex_nonwhitespace(value: &str) -> Result<bool, MojoError> {
    if value.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut found = -1_i64;
    status(unsafe {
        prodex_codex_auth_nonwhitespace_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut found as *mut i64) as usize as u64,
        )
    })?;
    match found {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexTerminalInput {
    TermProgram,
    Term,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum CodexTerminalKind {
    AppleTerminal = 0,
    Ghostty = 1,
    Iterm2 = 2,
    WarpTerminal = 3,
    VsCode = 4,
    WezTerm = 5,
    Kitty = 6,
    Alacritty = 7,
    Konsole = 8,
    GnomeTerminal = 9,
    Vte = 10,
    WindowsTerminal = 11,
    Dumb = 12,
    Unknown = 13,
}

/// Render and sanitize a terminal token in compiled Mojo. An explicitly empty
/// program still takes precedence over TERM and the detected terminal kind.
pub fn codex_terminal_token(
    kind: CodexTerminalKind,
    program: Option<&str>,
    version: Option<&str>,
    term: Option<&str>,
) -> Result<String, MojoError> {
    let fields = [
        program.unwrap_or_default(),
        version.unwrap_or_default(),
        term.unwrap_or_default(),
    ];
    if fields.iter().any(|value| value.len() > MAX_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = fields.map(|value| [value.as_ptr() as usize as u64, value.len() as u64]);
    let mut output = vec![0_u8; fields.iter().map(|value| value.len()).sum::<usize>() + 32];
    let mut written = -1_i64;
    status(unsafe {
        prodex_codex_auth_terminal_token_v1(
            ABI_VERSION,
            kind as i64,
            i64::from(program.is_some()),
            views.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let size = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if size > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(size);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

/// Classify the old TERM_PROGRAM normalization or exact TERM aliases in Mojo.
pub fn codex_terminal_kind(
    input: &str,
    source: CodexTerminalInput,
) -> Result<CodexTerminalKind, MojoError> {
    if input.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut result = -1_i64;
    status(unsafe {
        prodex_codex_auth_terminal_name_v1(
            ABI_VERSION,
            match source {
                CodexTerminalInput::TermProgram => 0,
                CodexTerminalInput::Term => 1,
            },
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    let kind = match result {
        0 => CodexTerminalKind::AppleTerminal,
        1 => CodexTerminalKind::Ghostty,
        2 => CodexTerminalKind::Iterm2,
        3 => CodexTerminalKind::WarpTerminal,
        4 => CodexTerminalKind::VsCode,
        5 => CodexTerminalKind::WezTerm,
        6 => CodexTerminalKind::Kitty,
        7 => CodexTerminalKind::Alacritty,
        8 => CodexTerminalKind::Konsole,
        9 => CodexTerminalKind::GnomeTerminal,
        10 => CodexTerminalKind::Vte,
        11 => CodexTerminalKind::WindowsTerminal,
        12 => CodexTerminalKind::Dumb,
        13 => CodexTerminalKind::Unknown,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(kind)
}

/// Select the first two whitespace-delimited tmux termtype tokens; host
/// only materializes checked original source ranges, never tokenizes again.
pub fn codex_term_program_parts(text: &str) -> Result<(String, Option<String>), MojoError> {
    if text.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-2_i64; 4];
    status(unsafe {
        prodex_codex_auth_terminal_parts_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let spans = output
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if *pair == [-1, -1] {
                return Ok(None);
            }
            let start = usize::try_from(pair[0]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(pair[1]).map_err(|_| MojoError::InvalidOutput)?;
            if start >= end {
                return Err(MojoError::InvalidOutput);
            }
            let text = text.get(start..end).ok_or(MojoError::InvalidOutput)?;
            Ok(Some((start, end, text)))
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    if spans[0].is_none() && spans[1].is_some() {
        return Err(MojoError::InvalidOutput);
    }
    if let (Some((_, first_end, _)), Some((second_start, _, _))) = (spans[0], spans[1])
        && first_end >= second_start
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok((
        spans[0]
            .map(|(_, _, value)| value.to_owned())
            .unwrap_or_default(),
        spans[1].map(|(_, _, value)| value.to_owned()),
    ))
}

#[cfg(test)]
#[path = "codex_auth_headers/terminal_token_tests.rs"]
mod terminal_token_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_codex_version_probe_retains_rightmost_candidate() {
        for (value, expected) in [
            ("codex-cli 0.142.5\n", Some("0.142.5")),
            ("codex-cli 1.2.3 5.6.7,", Some("5.6.7")),
            ("v1.2.3 2.3.4!", Some("2.3.4")),
            (" 0.142.5\u{2003}2.0.0-BETA❄ ", Some("2.0.0-BETA")),
            ("foo\t2.3.4-alpha.1", Some("2.3.4-alpha.1")),
            ("v1.2.3", None),
            ("unknown", None),
            ("", None),
        ] {
            assert_eq!(codex_probe_version(value), Ok(expected), "{value:?}");
        }
        assert_eq!(
            codex_probe_version(&"x".repeat(MAX_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_terminal_token_projection_and_nonspace_are_unicode_scalar_based() {
        assert_eq!(
            codex_terminal_header_token("Ghostty/1.2 α\t🧭"),
            Ok("Ghostty/1.2____".into())
        );
        assert_eq!(
            codex_terminal_header_token("xterm-256color"),
            Ok("xterm-256color".into())
        );
        assert_eq!(codex_terminal_header_token(""), Ok(String::new()));
        assert_eq!(codex_nonwhitespace(" \u{2003} \u{00a0}"), Ok(false));
        assert_eq!(codex_nonwhitespace(" \u{2003}α "), Ok(true));
        assert_eq!(codex_nonwhitespace(""), Ok(false));
    }

    #[test]
    fn compiled_codex_terminal_aliases_preserve_exact_modes() {
        use CodexTerminalInput::{Term, TermProgram};
        use CodexTerminalKind as K;
        for (name, expected) in [
            ("Apple_Terminal", K::AppleTerminal),
            ("  iTerm.app  ", K::Iterm2),
            ("I-TERM_2", K::Iterm2),
            ("WARP terminal", K::WarpTerminal),
            ("gnome-terminal", K::GnomeTerminal),
            ("Windows.Terminal", K::WindowsTerminal),
            ("Ghostty", K::Ghostty),
            ("kitty", K::Kitty),
            ("VsCode", K::VsCode),
            ("WezTerm", K::WezTerm),
            ("alacritty", K::Alacritty),
            ("Konsole", K::Konsole),
            ("VTE", K::Vte),
            ("DUMB", K::Dumb),
            ("unsupported", K::Unknown),
            (" Ghostty\t", K::Ghostty),
            ("\u{2003}Ghostty\u{00a0}", K::Ghostty),
            ("ghostty\tbroken", K::Unknown),
            ("ghostty\0", K::Unknown),
            ("日本語", K::Unknown),
        ] {
            assert_eq!(
                codex_terminal_kind(name, TermProgram),
                Ok(expected),
                "{name:?}"
            );
        }
        for (name, expected) in [
            ("wezterm", K::WezTerm),
            ("wezterm-mux", K::WezTerm),
            ("dumb", K::Dumb),
            ("DUMB", K::Unknown),
            ("ghostty", K::Unknown),
            ("wezterm\n", K::Unknown),
        ] {
            assert_eq!(codex_terminal_kind(name, Term), Ok(expected));
        }
    }

    #[test]
    fn compiled_codex_tmux_termtype_split_uses_unicode_whitespace() {
        assert_eq!(codex_term_program_parts(""), Ok(("".into(), None)));
        assert_eq!(
            codex_term_program_parts(" \u{2003}Ghostty\u{00a0}1.2.3 extra"),
            Ok(("Ghostty".into(), Some("1.2.3".into())))
        );
        assert_eq!(
            codex_term_program_parts("wezterm"),
            Ok(("wezterm".into(), None))
        );
        assert_eq!(
            codex_term_program_parts(" \u{2003} "),
            Ok(("".into(), None))
        );
        assert_eq!(
            codex_term_program_parts("日本語\u{2028}2.0"),
            Ok(("日本語".into(), Some("2.0".into())))
        );
    }

    #[test]
    fn compiled_codex_terminal_abi_rejects_malformed_input_without_writes() {
        let input = [0xff_u8];
        let mut output = [77_i64; 4];
        let address = output.as_mut_ptr() as usize as u64;
        for (version, mode, pointer, length, expected) in [
            (2, 0, 0, 0, 4),
            (1, 2, 0, 0, 1),
            (1, 0, 0, 1, 1),
            (1, 0, 0, -1, 1),
            (1, 0, input.as_ptr() as usize as u64, 1, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_codex_auth_terminal_name_v1(version, mode, pointer, length, address)
                },
                expected
            );
            assert_eq!(output, [77; 4]);
        }
        for (version, pointer, length, expected) in [
            (2, 0, 0, 4),
            (1, 0, 1, 1),
            (1, 0, -1, 1),
            (1, input.as_ptr() as usize as u64, 1, 1),
        ] {
            assert_eq!(
                unsafe { prodex_codex_auth_terminal_parts_v1(version, pointer, length, address) },
                expected
            );
            assert_eq!(output, [77; 4]);
        }
        let oversized = "x".repeat(MAX_BYTES + 1);
        assert_eq!(
            codex_terminal_kind(&oversized, CodexTerminalInput::Term),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            codex_term_program_parts(&oversized),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_codex_agent_projection_is_one_char_per_unicode_scalar() {
        assert_eq!(
            codex_user_agent_printable("hello world"),
            Ok("hello world".into())
        );
        assert_eq!(
            codex_user_agent_printable("Codex/1.0 🚀\tα\n"),
            Ok("Codex/1.0 ____".into())
        );
        assert_eq!(codex_user_agent_printable(""), Ok(String::new()));
        assert_eq!(
            codex_user_agent_printable(&"x".repeat(MAX_BYTES + 1)),
            Err(MojoError::InvalidInput)
        );
    }
}
