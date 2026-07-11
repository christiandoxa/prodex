use crate::RuntimeProxyCodexEndpoint;
use std::ffi::OsString;
use std::net::SocketAddr;

const PRODEX_CODEX_FULL_ACCESS_ARG: &str = "--full-access";
const PRODEX_DRY_RUN_ARG: &str = "--dry-run";
const CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG: &str = "--dangerously-bypass-approvals-and-sandbox";
const CODEX_PROFILE_ARG: &str = "--profile";
const CODEX_LEGACY_PROFILE_V2_ARG: &str = "--profile-v2";

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

pub fn normalize_run_codex_args(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(index) = first_codex_positional_arg_index(codex_args) else {
        return codex_args.to_vec();
    };
    let Some(first) = codex_args[index].to_str() else {
        return codex_args.to_vec();
    };
    if !looks_like_codex_session_id(first) {
        return codex_args.to_vec();
    }

    let mut normalized = Vec::with_capacity(codex_args.len() + 1);
    normalized.extend(codex_args[..index].iter().cloned());
    normalized.push(OsString::from("resume"));
    normalized.extend(codex_args[index..].iter().cloned());
    normalized
}

pub fn codex_resume_session_id(codex_args: &[OsString]) -> Option<&str> {
    let resume_index = codex_resume_command_index(codex_args)?;
    let resume_args = &codex_args[(resume_index + 1)..];
    let target_index = first_codex_positional_arg_index(resume_args)?;
    if resume_args[..target_index]
        .iter()
        .any(|arg| arg == "--last")
    {
        return None;
    }
    resume_args.get(target_index)?.to_str()
}

fn codex_resume_command_index(codex_args: &[OsString]) -> Option<usize> {
    let command_index = first_codex_positional_arg_index(codex_args)?;
    match codex_args.get(command_index)?.to_str()? {
        "resume" => Some(command_index),
        "exec" => {
            let after_exec = command_index + 1;
            let relative_index = first_codex_positional_arg_index(&codex_args[after_exec..])?;
            let resume_index = after_exec + relative_index;
            (codex_args.get(resume_index)?.to_str()? == "resume").then_some(resume_index)
        }
        _ => None,
    }
}

fn first_codex_positional_arg_index(codex_args: &[OsString]) -> Option<usize> {
    let mut index = 0;
    while index < codex_args.len() {
        let Some(arg) = codex_args[index].to_str() else {
            return Some(index);
        };
        if arg == "--" {
            return (index + 1 < codex_args.len()).then_some(index + 1);
        }
        if codex_option_takes_separate_value(arg) {
            index += 2;
            continue;
        }
        if codex_option_with_inline_value(arg) || codex_flag_option(arg) || arg.starts_with('-') {
            index += 1;
            continue;
        }
        return Some(index);
    }
    None
}

fn codex_option_takes_separate_value(arg: &str) -> bool {
    matches!(
        arg,
        "-c" | "--config"
            | "-i"
            | "--image"
            | "-m"
            | "--model"
            | "--local-provider"
            | "-p"
            | "--profile"
            | "-P"
            | "-s"
            | "--sandbox"
            | "-C"
            | "--cd"
            | "--add-dir"
            | "-a"
            | "--ask-for-approval"
            | "--enable"
            | "--disable"
            | "--remote"
            | "--remote-auth-token-env"
            | "--output-schema"
            | "-o"
            | "--output-last-message"
            | "--color"
    )
}

fn codex_option_with_inline_value(arg: &str) -> bool {
    if arg.starts_with("--") {
        return arg.contains('=');
    }
    ["-c", "-i", "-m", "-p", "-P", "-s", "-C", "-a", "-o"]
        .iter()
        .any(|prefix| arg.starts_with(prefix) && arg.len() > prefix.len())
}

fn codex_flag_option(arg: &str) -> bool {
    matches!(
        arg,
        "--oss"
            | "--search"
            | "--no-alt-screen"
            | "--dangerously-bypass-approvals-and-sandbox"
            | "--dangerously-bypass-hook-trust"
            | "--strict-config"
            | "--last"
            | "--all"
            | "--include-non-interactive"
            | "--skip-git-repo-check"
            | "--ephemeral"
            | "--ignore-user-config"
            | "--ignore-rules"
            | "--json"
            | "-h"
            | "--help"
            | "-V"
            | "--version"
    )
}

fn looks_like_codex_session_id(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 5 {
        return false;
    }
    let expected_lengths = [8usize, 4, 4, 4, 12];
    parts.iter().zip(expected_lengths).all(|(part, expected)| {
        part.len() == expected && part.chars().all(|ch| ch.is_ascii_hexdigit())
    })
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
    let provider_base_override =
        format!("{}={}", provider_base_key, toml_string_literal(&proxy_base));
    let mut args = Vec::with_capacity(user_args.len() + 2);
    let mut replaced = false;
    let mut index = 0;
    while index < user_args.len() {
        let Some(arg) = user_args[index].to_str() else {
            args.push(user_args[index].clone());
            index += 1;
            continue;
        };

        if matches!(arg, "-c" | "--config") {
            args.push(user_args[index].clone());
            index += 1;
            if index < user_args.len() {
                if config_assignment_key(user_args[index].to_str())
                    == Some(provider_base_key.as_str())
                {
                    args.push(OsString::from(provider_base_override.clone()));
                    replaced = true;
                } else {
                    args.push(user_args[index].clone());
                }
            }
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--config=") {
            if config_assignment_key(Some(value)) == Some(provider_base_key.as_str()) {
                args.push(OsString::from(format!("--config={provider_base_override}")));
                replaced = true;
            } else {
                args.push(user_args[index].clone());
            }
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("-c")
            && !value.is_empty()
            && value.contains('=')
        {
            if config_assignment_key(Some(value)) == Some(provider_base_key.as_str()) {
                args.push(OsString::from(format!("-c{provider_base_override}")));
                replaced = true;
            } else {
                args.push(user_args[index].clone());
            }
            index += 1;
            continue;
        }

        args.push(user_args[index].clone());
        index += 1;
    }

    if !replaced {
        args.push(OsString::from("-c"));
        args.push(OsString::from(provider_base_override));
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

    let mut args = Vec::with_capacity(user_args.len() + (overrides.len() * 2));
    let mut replaced = vec![false; overrides.len()];
    let mut index = 0;
    while index < user_args.len() {
        let Some(arg) = user_args[index].to_str() else {
            args.push(user_args[index].clone());
            index += 1;
            continue;
        };

        if (arg == "-c" || arg == "--config")
            && let Some(next) = user_args.get(index + 1)
        {
            if let Some((override_index, override_value)) =
                runtime_proxy_matching_realtime_override(next.to_str(), &overrides)
            {
                args.push(user_args[index].clone());
                args.push(OsString::from(override_value));
                replaced[override_index] = true;
            } else {
                args.push(user_args[index].clone());
                args.push(next.clone());
            }
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--config=") {
            if let Some((override_index, override_value)) =
                runtime_proxy_matching_realtime_override(Some(value), &overrides)
            {
                args.push(OsString::from(format!("--config={override_value}")));
                replaced[override_index] = true;
            } else {
                args.push(user_args[index].clone());
            }
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("-c")
            && !value.is_empty()
            && value.contains('=')
        {
            if let Some((override_index, override_value)) =
                runtime_proxy_matching_realtime_override(Some(value), &overrides)
            {
                args.push(OsString::from(format!("-c{override_value}")));
                replaced[override_index] = true;
            } else {
                args.push(user_args[index].clone());
            }
            index += 1;
            continue;
        }

        args.push(user_args[index].clone());
        index += 1;
    }

    for (index, (key, value)) in overrides.iter().enumerate() {
        if !replaced[index] {
            args.push(OsString::from("-c"));
            args.push(OsString::from(format!("{key}={value}")));
        }
    }
    args
}

fn runtime_proxy_matching_realtime_override(
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
    codex_args: &[OsString],
    full_access_requested: bool,
) -> (Vec<OsString>, bool) {
    let (passthrough_full_access, codex_args) = extract_prodex_full_access_flag(codex_args);
    let codex_args = normalize_codex_profile_args(&codex_args);
    let codex_args = normalize_run_codex_args(&codex_args);
    let include_code_review = is_review_invocation(&codex_args);
    let codex_args = codex_launch_args_with_full_access(
        &codex_args,
        full_access_requested || passthrough_full_access,
    );
    (codex_args, include_code_review)
}

pub fn scope_codex_exec_config_args(codex_args: &[OsString]) -> Vec<OsString> {
    let codex_args = scope_codex_exec_config_args_to_exec(codex_args);
    scope_codex_exec_resume_config_args_to_resume(&codex_args)
}

fn scope_codex_exec_config_args_to_exec(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(exec_index) = first_codex_positional_arg_index(codex_args) else {
        return codex_args.to_vec();
    };
    if codex_args.get(exec_index).and_then(|arg| arg.to_str()) != Some("exec") {
        return codex_args.to_vec();
    }

    let mut prefix = Vec::with_capacity(exec_index);
    let mut config_overrides = Vec::new();
    let mut index = 0;
    while index < exec_index {
        let Some(arg) = codex_args[index].to_str() else {
            prefix.push(codex_args[index].clone());
            index += 1;
            continue;
        };

        if matches!(arg, "-c" | "--config") && index + 1 < exec_index {
            config_overrides.push(codex_args[index].clone());
            config_overrides.push(codex_args[index + 1].clone());
            index += 2;
            continue;
        }

        if is_inline_config_override_arg(arg) {
            config_overrides.push(codex_args[index].clone());
            index += 1;
            continue;
        }

        prefix.push(codex_args[index].clone());
        index += 1;
    }

    if config_overrides.is_empty() {
        return codex_args.to_vec();
    }

    let mut args = Vec::with_capacity(codex_args.len());
    args.extend(prefix);
    args.push(codex_args[exec_index].clone());
    args.extend(config_overrides);
    args.extend(codex_args[(exec_index + 1)..].iter().cloned());
    args
}

fn scope_codex_exec_resume_config_args_to_resume(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(exec_index) = first_codex_positional_arg_index(codex_args) else {
        return codex_args.to_vec();
    };
    if codex_args.get(exec_index).and_then(|arg| arg.to_str()) != Some("exec") {
        return codex_args.to_vec();
    }

    let after_exec = exec_index + 1;
    let Some(relative_resume_index) = first_codex_positional_arg_index(&codex_args[after_exec..])
    else {
        return codex_args.to_vec();
    };
    let resume_index = after_exec + relative_resume_index;
    if codex_args.get(resume_index).and_then(|arg| arg.to_str()) != Some("resume") {
        return codex_args.to_vec();
    }

    let mut exec_options = Vec::with_capacity(resume_index.saturating_sub(after_exec));
    let mut config_overrides = Vec::new();
    let mut index = after_exec;
    while index < resume_index {
        let Some(arg) = codex_args[index].to_str() else {
            exec_options.push(codex_args[index].clone());
            index += 1;
            continue;
        };

        if matches!(arg, "-c" | "--config") && index + 1 < resume_index {
            config_overrides.push(codex_args[index].clone());
            config_overrides.push(codex_args[index + 1].clone());
            index += 2;
            continue;
        }

        if is_inline_config_override_arg(arg) {
            config_overrides.push(codex_args[index].clone());
            index += 1;
            continue;
        }

        exec_options.push(codex_args[index].clone());
        index += 1;
    }

    if config_overrides.is_empty() {
        return codex_args.to_vec();
    }

    let mut args = Vec::with_capacity(codex_args.len());
    args.extend(codex_args[..=exec_index].iter().cloned());
    args.extend(exec_options);
    args.push(codex_args[resume_index].clone());
    args.extend(config_overrides);
    args.extend(codex_args[(resume_index + 1)..].iter().cloned());
    args
}

pub fn normalize_codex_profile_args(codex_args: &[OsString]) -> Vec<OsString> {
    codex_args
        .iter()
        .map(|arg| {
            let Some(value) = arg.to_str() else {
                return arg.clone();
            };
            if value == CODEX_LEGACY_PROFILE_V2_ARG {
                return OsString::from(CODEX_PROFILE_ARG);
            }
            if let Some(profile) = value.strip_prefix("--profile-v2=") {
                return OsString::from(format!("{CODEX_PROFILE_ARG}={profile}"));
            }
            arg.clone()
        })
        .collect()
}

pub fn extract_prodex_dry_run_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut dry_run = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_DRY_RUN_ARG {
            dry_run = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (dry_run, filtered)
}

pub fn prodex_dry_run_requested(codex_args: &[OsString]) -> bool {
    codex_args.iter().any(|arg| arg == PRODEX_DRY_RUN_ARG)
}

pub fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

fn extract_prodex_full_access_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut full_access = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_CODEX_FULL_ACCESS_ARG {
            full_access = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (full_access, filtered)
}

fn is_inline_config_override_arg(arg: &str) -> bool {
    arg.strip_prefix("--config=")
        .or_else(|| arg.strip_prefix("-c"))
        .is_some_and(|value| value.contains('='))
}

fn codex_launch_args_with_full_access(codex_args: &[OsString], full_access: bool) -> Vec<OsString> {
    if !full_access {
        return codex_args.to_vec();
    }

    let mut args = Vec::with_capacity(codex_args.len() + 1);
    args.push(OsString::from(CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG));
    args.extend(codex_args.iter().cloned());
    args
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
