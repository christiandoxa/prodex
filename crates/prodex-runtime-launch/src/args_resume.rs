use super::args::{
    extend_without_codex_positionals, extend_without_codex_thread_source,
    first_codex_positional_arg_index,
};
use std::ffi::OsString;

/// Replace a fresh `codex exec` invocation with the upstream-supported
/// `codex exec resume <session-id>` form while retaining global options.
pub fn retarget_codex_exec_resume_args(codex_args: &[OsString], session_id: &str) -> Vec<OsString> {
    let positional_index = first_codex_positional_arg_index(codex_args);
    let command_index = positional_index
        .or_else(|| codex_args.iter().position(|arg| arg == "--"))
        .unwrap_or(codex_args.len());
    let mut args = Vec::with_capacity(codex_args.len() + 2);
    extend_without_resume_last(&mut args, &codex_args[..command_index]);
    args.extend([
        OsString::from("exec"),
        OsString::from("resume"),
        OsString::from(session_id),
    ]);
    if positional_index.is_some() {
        let mut preserved = Vec::new();
        extend_without_codex_positionals(
            &mut preserved,
            codex_args
                .get(command_index.saturating_add(1)..)
                .unwrap_or_default(),
        );
        args.extend(preserved.into_iter().filter(|arg| arg != "--last"));
    }
    args
}

fn extend_without_resume_last(output: &mut Vec<OsString>, args: &[OsString]) {
    let mut preserved = Vec::with_capacity(args.len());
    extend_without_codex_thread_source(&mut preserved, args);
    output.extend(preserved.into_iter().filter(|arg| arg != "--last"));
}
