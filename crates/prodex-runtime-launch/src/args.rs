use crate::RuntimeProxyCodexEndpoint;
use prodex_mojo_core::launch::LaunchArgumentOperation;
use runtime_proxy_crate as runtime_proxy;
use std::ffi::OsString;
use std::net::SocketAddr;

const PRODEX_GOVERNED_HTTP_PROVIDER_ID: &str = "prodex-openai-governed-http";

pub fn runtime_proxy_codex_passthrough_args(
    runtime_proxy: Option<RuntimeProxyCodexEndpoint<'_>>,
    user_args: &[OsString],
) -> Vec<OsString> {
    runtime_proxy
        .map(|proxy| {
            let args = if let Some(local_provider_id) = proxy.local_model_provider_id {
                runtime_proxy_local_model_provider_codex_args(
                    proxy.listen_addr,
                    proxy.openai_mount_path,
                    local_provider_id,
                    user_args,
                )
            } else if proxy.force_http_responses {
                runtime_proxy_governed_http_codex_args(
                    proxy.listen_addr,
                    proxy.openai_mount_path,
                    user_args,
                )
            } else if proxy.openai_mount_path == runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH {
                runtime_proxy_codex_args(proxy.listen_addr, user_args)
            } else {
                runtime_proxy_codex_args_with_mount_path(
                    proxy.listen_addr,
                    proxy.openai_mount_path,
                    user_args,
                )
            };
            runtime_proxy_realtime_codex_args(
                proxy.realtime_ws_base_url,
                proxy.realtime_ws_model,
                &args,
            )
        })
        .unwrap_or_else(|| user_args.to_vec())
}

fn runtime_proxy_governed_http_codex_args(
    listen_addr: SocketAddr,
    openai_mount_path: &str,
    user_args: &[OsString],
) -> Vec<OsString> {
    let mut args = scope_codex_exec_config_args(&runtime_proxy_codex_args_with_mount_path(
        listen_addr,
        openai_mount_path,
        user_args,
    ));
    let proxy_openai_base = format!(
        "http://{listen_addr}{}",
        normalize_mount_path(openai_mount_path)
    );
    let overrides = [
        format!(
            "model_provider={}",
            toml_string_literal(PRODEX_GOVERNED_HTTP_PROVIDER_ID)
        ),
        format!(
            "model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.name={}",
            toml_string_literal("OpenAI through Prodex governance")
        ),
        format!(
            "model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.base_url={}",
            toml_string_literal(&proxy_openai_base)
        ),
        format!("model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.wire_api=\"responses\""),
        format!("model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.requires_openai_auth=true"),
        format!("model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.supports_websockets=false"),
        format!(
            "model_providers.{PRODEX_GOVERNED_HTTP_PROVIDER_ID}.supports_standalone_web_search=true"
        ),
    ];
    let insert_at = governed_http_config_insertion_index(&args);
    args.splice(
        insert_at..insert_at,
        overrides
            .into_iter()
            .flat_map(|value| [OsString::from("-c"), OsString::from(value)]),
    );
    args
}

fn governed_http_config_insertion_index(args: &[OsString]) -> usize {
    super::args_mojo::inspect(args).governed_insertion
}

pub fn normalize_run_codex_args(args: &[OsString]) -> Vec<OsString> {
    super::args_mojo::plan(args, LaunchArgumentOperation::NormalizeRun, false, None).0
}

pub fn codex_resume_session_id(args: &[OsString]) -> Option<&str> {
    super::args_mojo::inspect(args).resume_session
}

pub fn codex_resume_requested(args: &[OsString]) -> bool {
    super::args_mojo::inspect(args).resume_command.is_some()
}

pub fn retarget_codex_tui_resume_args(args: &[OsString], session_id: &str) -> Vec<OsString> {
    super::args_mojo::plan(
        args,
        LaunchArgumentOperation::RetargetTui,
        false,
        Some(session_id),
    )
    .0
}

pub fn is_codex_exec_invocation(args: &[OsString]) -> bool {
    super::args_mojo::inspect(args).is_exec
}

pub fn runtime_launch_cli_model(args: &[OsString]) -> Option<String> {
    super::args_mojo::inspect(args).model.map(str::to_owned)
}

pub fn runtime_proxy_codex_args(listen_addr: SocketAddr, user_args: &[OsString]) -> Vec<OsString> {
    runtime_proxy_codex_args_with_mount_path(
        listen_addr,
        runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH,
        user_args,
    )
}

pub fn runtime_proxy_codex_args_with_mount_path(
    listen_addr: SocketAddr,
    openai_mount_path: &str,
    user_args: &[OsString],
) -> Vec<OsString> {
    let proxy_chatgpt_base = format!("http://{listen_addr}/backend-api");
    let proxy_openai_base = format!("http://{listen_addr}{openai_mount_path}");
    let overrides = [
        format!(
            "chatgpt_base_url={}",
            toml_string_literal(&proxy_chatgpt_base)
        ),
        format!(
            "openai_base_url={}",
            toml_string_literal(&proxy_openai_base),
        ),
    ];

    let mut args = Vec::with_capacity((overrides.len() * 2) + user_args.len());
    for override_entry in overrides {
        args.push(OsString::from("-c"));
        args.push(OsString::from(override_entry));
    }
    args.extend(user_args.iter().cloned());
    args
}

pub fn runtime_proxy_local_model_provider_codex_args(
    listen_addr: SocketAddr,
    mount_path: &str,
    local_provider_id: &str,
    user_args: &[OsString],
) -> Vec<OsString> {
    let proxy_base = format!("http://{listen_addr}{}", normalize_mount_path(mount_path));
    let provider_base_key = format!("model_providers.{local_provider_id}.base_url");
    let overrides = [(provider_base_key, toml_string_literal(&proxy_base))];
    let (mut args, replaced) = rewrite_codex_config_overrides(user_args, &overrides);
    if !replaced[0] {
        let insert_at = codex_config_override_insertion_index(&args);
        args.splice(
            insert_at..insert_at,
            [
                OsString::from("-c"),
                OsString::from(format!("{}={}", overrides[0].0, overrides[0].1)),
            ],
        );
    }
    args
}

fn runtime_proxy_realtime_codex_args(
    realtime_ws_base_url: Option<&str>,
    realtime_ws_model: Option<&str>,
    user_args: &[OsString],
) -> Vec<OsString> {
    let mut overrides = Vec::new();
    if let Some(base_url) = realtime_ws_base_url.filter(|value| !value.trim().is_empty()) {
        overrides.push((
            "experimental_realtime_ws_base_url".to_string(),
            toml_string_literal(base_url),
        ));
    }
    if let Some(model) = realtime_ws_model.filter(|value| !value.trim().is_empty()) {
        overrides.push((
            "experimental_realtime_ws_model".to_string(),
            toml_string_literal(model),
        ));
    }
    if overrides.is_empty() {
        return user_args.to_vec();
    }

    let (mut args, replaced) = rewrite_codex_config_overrides(user_args, &overrides);

    for (index, (key, value)) in overrides.iter().enumerate() {
        if !replaced[index] {
            let insert_at = codex_config_override_insertion_index(&args);
            args.splice(
                insert_at..insert_at,
                [
                    OsString::from("-c"),
                    OsString::from(format!("{key}={value}")),
                ],
            );
        }
    }
    args
}

enum CodexConfigArg<'a> {
    Stop,
    Separate(&'a OsString),
    LongInline(&'a str),
    ShortInline(&'a str),
    Other,
}

fn codex_config_arg(args: &[OsString], index: usize) -> CodexConfigArg<'_> {
    let Some(arg) = args[index].to_str() else {
        return CodexConfigArg::Other;
    };
    if arg == "--" {
        return CodexConfigArg::Stop;
    }
    if matches!(arg, "-c" | "--config")
        && let Some(value) = args.get(index + 1)
    {
        return CodexConfigArg::Separate(value);
    }
    if let Some(value) = arg.strip_prefix("--config=") {
        return CodexConfigArg::LongInline(value);
    }
    if let Some(value) = arg.strip_prefix("-c")
        && !value.is_empty()
        && value.contains('=')
    {
        return CodexConfigArg::ShortInline(value);
    }
    CodexConfigArg::Other
}

fn rewrite_codex_config_overrides(
    user_args: &[OsString],
    overrides: &[(String, String)],
) -> (Vec<OsString>, Vec<bool>) {
    let mut args = Vec::with_capacity(user_args.len() + (overrides.len() * 2));
    let mut replaced = vec![false; overrides.len()];
    let mut index = 0;
    while index < user_args.len() {
        match codex_config_arg(user_args, index) {
            CodexConfigArg::Stop => {
                args.extend(user_args[index..].iter().cloned());
                break;
            }
            CodexConfigArg::Separate(value) => {
                args.push(user_args[index].clone());
                args.push(rewrite_codex_config_assignment(
                    value.to_str(),
                    overrides,
                    &mut replaced,
                    "",
                    value,
                ));
                index += 2;
            }
            CodexConfigArg::LongInline(value) => {
                args.push(rewrite_codex_config_assignment(
                    Some(value),
                    overrides,
                    &mut replaced,
                    "--config=",
                    &user_args[index],
                ));
                index += 1;
            }
            CodexConfigArg::ShortInline(value) => {
                args.push(rewrite_codex_config_assignment(
                    Some(value),
                    overrides,
                    &mut replaced,
                    "-c",
                    &user_args[index],
                ));
                index += 1;
            }
            CodexConfigArg::Other => {
                args.push(user_args[index].clone());
                index += 1;
            }
        }
    }
    (args, replaced)
}

fn rewrite_codex_config_assignment(
    assignment: Option<&str>,
    overrides: &[(String, String)],
    replaced: &mut [bool],
    prefix: &str,
    fallback: &OsString,
) -> OsString {
    let Some((index, value)) = runtime_proxy_matching_config_override(assignment, overrides) else {
        return fallback.clone();
    };
    replaced[index] = true;
    OsString::from(format!("{prefix}{value}"))
}

fn codex_config_override_insertion_index(args: &[OsString]) -> usize {
    super::args_mojo::inspect(args).config_insertion
}

fn runtime_proxy_matching_config_override(
    assignment: Option<&str>,
    overrides: &[(String, String)],
) -> Option<(usize, String)> {
    let key = config_assignment_key(assignment)?;
    overrides
        .iter()
        .enumerate()
        .find(|(_, (override_key, _))| override_key == key)
        .map(|(index, (key, value))| (index, format!("{key}={value}")))
}

fn normalize_mount_path(mount_path: &str) -> String {
    let trimmed = mount_path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return String::new();
    }
    format!("/{}", trimmed.trim_matches('/'))
}

fn config_assignment_key(assignment: Option<&str>) -> Option<&str> {
    assignment
        .and_then(|assignment| assignment.split_once('='))
        .map(|(key, _)| key.trim())
        .filter(|key| !key.is_empty())
}

pub fn prepare_codex_launch_args(
    args: &[OsString],
    full_access_requested: bool,
) -> (Vec<OsString>, bool) {
    super::args_mojo::plan(
        args,
        LaunchArgumentOperation::Prepare,
        full_access_requested,
        None,
    )
}

pub fn scope_codex_exec_config_args(args: &[OsString]) -> Vec<OsString> {
    super::args_mojo::plan(args, LaunchArgumentOperation::ScopeConfig, false, None).0
}

pub fn normalize_codex_profile_args(args: &[OsString]) -> Vec<OsString> {
    super::args_mojo::plan(args, LaunchArgumentOperation::NormalizeProfile, false, None).0
}

pub fn extract_prodex_dry_run_flag(args: &[OsString]) -> (bool, Vec<OsString>) {
    let (args, flag) =
        super::args_mojo::plan(args, LaunchArgumentOperation::ExtractDryRun, false, None);
    (flag, args)
}

pub fn prodex_dry_run_requested(args: &[OsString]) -> bool {
    super::args_mojo::inspect(args).dry_run
}

pub fn is_review_invocation(args: &[OsString]) -> bool {
    super::args_mojo::inspect(args).is_review
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
