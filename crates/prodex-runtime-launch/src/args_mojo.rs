//! Native OS-string ownership and validated plan reconstruction only.
use prodex_mojo_core::launch::{
    self, LaunchArgument, LaunchArgumentInspection, LaunchArgumentOperation,
};
use std::ffi::OsString;

pub(super) fn inspect(args: &[OsString]) -> LaunchArgumentInspection<'_> {
    let views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    launch::inspect_launch_arguments(&views)
        .expect("Mojo Codex argument inspection returned invalid output")
}

pub(super) fn plan(
    args: &[OsString],
    operation: LaunchArgumentOperation,
    full_access: bool,
    session: Option<&str>,
) -> (Vec<OsString>, bool) {
    let views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    let plan = launch::plan_launch_arguments(&views, operation, full_access)
        .expect("Mojo Codex argument planning returned invalid output");
    let output = plan
        .arguments
        .into_iter()
        .map(|arg| match arg {
            LaunchArgument::Original(index) => args[index].clone(),
            LaunchArgument::Profile => OsString::from("--profile"),
            LaunchArgument::ProfileInline { index, offset } => {
                let value = args[index].to_str().expect("validated UTF-8 argument");
                OsString::from(format!("--profile={}", &value[offset..]))
            }
            LaunchArgument::Resume => OsString::from("resume"),
            LaunchArgument::Exec => OsString::from("exec"),
            LaunchArgument::Session => {
                OsString::from(session.expect("retarget operation supplies a session"))
            }
            LaunchArgument::FullAccess => {
                OsString::from("--dangerously-bypass-approvals-and-sandbox")
            }
        })
        .collect();
    (output, plan.flag)
}

pub(super) fn rewrite_config(
    args: &[OsString],
    overrides: &[(String, String)],
) -> (Vec<OsString>, Vec<bool>) {
    use prodex_mojo_core::launch_config::{LaunchConfigArgument, plan_launch_config};
    let views = args.iter().map(|arg| arg.to_str()).collect::<Vec<_>>();
    let keys = overrides
        .iter()
        .map(|(key, _)| key.as_str())
        .collect::<Vec<_>>();
    let plan = plan_launch_config(&views, &keys)
        .expect("Mojo Codex config planning returned invalid output");
    let output = plan
        .arguments
        .into_iter()
        .zip(args)
        .map(|(item, original)| {
            let (prefix, index) = match item {
                LaunchConfigArgument::Original => return original.clone(),
                LaunchConfigArgument::Separate(index) => ("", index),
                LaunchConfigArgument::LongInline(index) => ("--config=", index),
                LaunchConfigArgument::ShortInline(index) => ("-c", index),
            };
            let (key, value) = &overrides[index];
            OsString::from(format!("{prefix}{key}={value}"))
        })
        .collect();
    (output, plan.replaced)
}
