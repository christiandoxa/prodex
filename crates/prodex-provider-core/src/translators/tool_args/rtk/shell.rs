//! RTK shell-command ABI adapters.

pub(crate) fn rtk_prefixed_noisy_shell_command(command: &str) -> Option<String> {
    prodex_mojo_core::rtk_noisy::prefixed_noisy_shell_command(command)
        .unwrap_or_else(|error| panic!("Mojo RTK rewrite failed: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::rtk_prefixed_noisy_shell_command;

    #[test]
    fn prefixed_contract_preserves_quotes_and_separator_precedence() {
        assert_eq!(
            rtk_prefixed_noisy_shell_command("printf 'a; b' && cargo test"),
            Some("rtk printf 'a; b' && cargo test".to_string())
        );
        assert_eq!(
            rtk_prefixed_noisy_shell_command("pwd||cargo test"),
            Some("rtk pwd||cargo test".to_string())
        );
        assert_eq!(rtk_prefixed_noisy_shell_command("rtk cargo test"), None);
    }
}
