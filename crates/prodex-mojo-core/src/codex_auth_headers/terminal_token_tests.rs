use super::*;

#[test]
fn compiled_terminal_token_preserves_all_labels_and_version_rules() {
    use CodexTerminalKind as K;
    for (kind, label, versioned) in [
        (K::AppleTerminal, "Apple_Terminal", "Apple_Terminal/1.2"),
        (K::Ghostty, "Ghostty", "Ghostty/1.2"),
        (K::Iterm2, "iTerm.app", "iTerm.app/1.2"),
        (K::WarpTerminal, "WarpTerminal", "WarpTerminal/1.2"),
        (K::VsCode, "vscode", "vscode/1.2"),
        (K::WezTerm, "WezTerm", "WezTerm/1.2"),
        (K::Kitty, "kitty", "kitty"),
        (K::Alacritty, "Alacritty", "Alacritty"),
        (K::Konsole, "Konsole", "Konsole/1.2"),
        (K::GnomeTerminal, "gnome-terminal", "gnome-terminal"),
        (K::Vte, "VTE", "VTE/1.2"),
        (K::WindowsTerminal, "WindowsTerminal", "WindowsTerminal"),
        (K::Dumb, "dumb", "dumb"),
        (K::Unknown, "unknown", "unknown"),
    ] {
        assert_eq!(
            codex_terminal_token(kind, None, None, None).as_deref(),
            Ok(label)
        );
        assert_eq!(
            codex_terminal_token(kind, None, Some(""), Some("")).as_deref(),
            Ok(label)
        );
        assert_eq!(
            codex_terminal_token(kind, None, Some("1.2"), None).as_deref(),
            Ok(versioned)
        );
    }
}

#[test]
fn compiled_terminal_token_preserves_precedence_and_scalar_sanitizing() {
    use CodexTerminalKind as K;
    for (program, version, term, expected) in [
        (Some("Program"), Some("2"), Some("term"), "Program/2"),
        (Some(""), None, Some("term"), ""),
        (Some(""), Some("2"), Some("term"), "/2"),
        (Some("Program"), Some(""), Some("term"), "Program"),
        (None, Some("2"), Some("xterm-256color"), "xterm-256color"),
        (None, Some("2"), Some(""), "Ghostty/2"),
        (Some("AZaz09-_./"), None, None, "AZaz09-_./"),
        (Some("a🦀é\u{301}\n\0z"), Some("β 1"), None, "a_____z/__1"),
    ] {
        assert_eq!(
            codex_terminal_token(K::Ghostty, program, version, term).as_deref(),
            Ok(expected)
        );
    }
}

#[test]
fn compiled_terminal_token_abi_is_atomic_on_invalid_input_and_short_output() {
    let fields = [[0_u64; 2]; 3];
    let mut output = [77_u8; 32];
    let mut written = 91_i64;
    for (abi, kind, present, address, capacity, expected) in [
        (2, 0, 0, 0, 32, 4),
        (1, -1, 0, fields.as_ptr() as usize as u64, 32, 1),
        (1, 14, 0, fields.as_ptr() as usize as u64, 32, 1),
        (1, 0, 2, fields.as_ptr() as usize as u64, 32, 1),
        (1, 0, 0, 0, 32, 1),
        (1, 0, 0, fields.as_ptr() as usize as u64 + 1, 32, 1),
        (1, 0, 0, fields.as_ptr() as usize as u64, -1, 1),
        (1, 0, 0, fields.as_ptr() as usize as u64, 1, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_codex_auth_terminal_token_v1(
                    abi,
                    kind,
                    present,
                    address,
                    output.as_mut_ptr() as usize as u64,
                    capacity,
                    (&mut written as *mut i64) as usize as u64,
                )
            },
            expected
        );
        assert_eq!(output, [77; 32]);
        assert_eq!(written, 91);
    }
    let malformed = [0xff_u8];
    let bad_fields = [[malformed.as_ptr() as usize as u64, 1], [0, 0], [0, 0]];
    assert_eq!(
        unsafe {
            prodex_codex_auth_terminal_token_v1(
                1,
                0,
                1,
                bad_fields.as_ptr() as usize as u64,
                output.as_mut_ptr() as usize as u64,
                32,
                (&mut written as *mut i64) as usize as u64,
            )
        },
        1
    );
    assert_eq!(output, [77; 32]);
    assert_eq!(written, 91);
    assert_eq!(
        unsafe {
            prodex_codex_auth_terminal_token_v1(
                1,
                0,
                1,
                fields.as_ptr() as usize as u64,
                0,
                0,
                (&mut written as *mut i64) as usize as u64,
            )
        },
        0
    );
    assert_eq!(written, 0);
    let oversized = "x".repeat(MAX_BYTES + 1);
    assert_eq!(
        codex_terminal_token(CodexTerminalKind::Unknown, Some(&oversized), None, None),
        Err(MojoError::InvalidInput)
    );
}
