pub const RUNTIME_PROXY_DOCTOR_TAIL_BYTES: usize = 128 * 1024;

pub(crate) use prodex_mojo_core::cli_presentation::CliHelpKind;

pub(crate) fn cli_after_help(kind: CliHelpKind) -> String {
    prodex_mojo_core::cli_presentation::help_text(kind)
        .expect("compiled Mojo CLI help renderer returned invalid output")
}
