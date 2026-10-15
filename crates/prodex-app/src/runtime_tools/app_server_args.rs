use prodex_mojo_core::runtime_overlay_policy::AppServerConfigAction;
use std::ffi::OsString;

pub(super) fn project_app_server_config_args(
    runtime_args: &[OsString],
    include_inline_config: bool,
) -> Vec<OsString> {
    let lossy = runtime_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let arguments = lossy
        .iter()
        .map(|arg| Some(arg.as_str()))
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::runtime_overlay_policy::app_server_config_plan(
        &arguments,
        include_inline_config,
    )
    .expect("Mojo app-server config projection returned invalid output");
    let mut args = Vec::with_capacity(runtime_args.len());
    for action in plan {
        match action {
            AppServerConfigAction::Keep(index) => args.push(runtime_args[index].clone()),
            AppServerConfigAction::KeepPair(index) => {
                args.extend([runtime_args[index].clone(), runtime_args[index + 1].clone()]);
            }
            AppServerConfigAction::ModelPair { value, .. } => {
                let Some(value) = runtime_args[value].to_str() else {
                    continue;
                };
                args.extend([
                    OsString::from("-c"),
                    OsString::from(format!(
                        "model={}",
                        crate::runtime_catalog_config::toml_string_literal(value)
                    )),
                ]);
            }
            AppServerConfigAction::ModelInline { index, value_start } => {
                let value = lossy[index]
                    .get(value_start..)
                    .expect("Mojo app-server model offset must be a UTF-8 boundary");
                args.extend([
                    OsString::from("-c"),
                    OsString::from(format!(
                        "model={}",
                        crate::runtime_catalog_config::toml_string_literal(value)
                    )),
                ]);
            }
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_server_projection_preserves_hook_and_companion_argument_shapes() {
        let input = [
            OsString::from("--model"),
            OsString::from("gpt-6.1-sol"),
            OsString::from("--config=projects={a=1}"),
            OsString::from("--strict-config"),
            OsString::from("--enable"),
            OsString::from("web_search"),
            OsString::from("resume"),
        ];
        assert_eq!(
            project_app_server_config_args(&input, true),
            vec![
                OsString::from("-c"),
                OsString::from("model=\"gpt-6.1-sol\""),
                OsString::from("--config=projects={a=1}"),
                OsString::from("--strict-config"),
                OsString::from("--enable"),
                OsString::from("web_search"),
            ]
        );
        assert_eq!(
            project_app_server_config_args(&input, false),
            vec![
                OsString::from("-c"),
                OsString::from("model=\"gpt-6.1-sol\""),
                OsString::from("--strict-config"),
                OsString::from("--enable"),
                OsString::from("web_search"),
            ]
        );
    }

    #[test]
    fn app_server_projection_drops_non_utf8_model_pair_values() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let input = [
                OsString::from("--model"),
                OsString::from_vec(vec![0xff]),
                OsString::from("--strict-config"),
            ];
            assert_eq!(
                project_app_server_config_args(&input, true),
                vec![OsString::from("--strict-config")]
            );
        }
    }
}
