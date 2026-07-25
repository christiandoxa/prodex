use std::ffi::OsString;
use std::path::Path;

pub(crate) fn trusted_workspace_codex_args(
    workspace: &Path,
    codex_args: &[OsString],
) -> Vec<OsString> {
    let workspace = serde_json::to_string(&workspace.to_string_lossy())
        .expect("workspace path should serialize as a TOML-compatible string");
    let mut args = Vec::with_capacity(codex_args.len() + 3);
    args.push(OsString::from("-c"));
    args.push(OsString::from(format!(
        "projects={{{workspace}={{trust_level=\"trusted\"}}}}"
    )));
    args.push(OsString::from("--dangerously-bypass-hook-trust"));
    args.extend(codex_args.iter().cloned());
    args
}
