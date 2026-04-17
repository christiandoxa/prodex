use super::*;

pub(crate) fn runtime_proxy_codex_passthrough_args(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    user_args: &[OsString],
) -> Vec<OsString> {
    runtime_proxy
        .map(|proxy| {
            if proxy.openai_mount_path == RUNTIME_PROXY_OPENAI_MOUNT_PATH {
                runtime_proxy_codex_args(proxy.listen_addr, user_args)
            } else {
                runtime_proxy_codex_args_with_mount_path(
                    proxy.listen_addr,
                    &proxy.openai_mount_path,
                    user_args,
                )
            }
        })
        .unwrap_or_else(|| user_args.to_vec())
}

pub(crate) fn normalize_run_codex_args(codex_args: &[OsString]) -> Vec<OsString> {
    let Some(first) = codex_args.first().and_then(|arg| arg.to_str()) else {
        return codex_args.to_vec();
    };
    if !looks_like_codex_session_id(first) {
        return codex_args.to_vec();
    }

    let mut normalized = Vec::with_capacity(codex_args.len() + 1);
    normalized.push(OsString::from("resume"));
    normalized.extend(codex_args.iter().cloned());
    normalized
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

pub(crate) fn runtime_proxy_codex_args(
    listen_addr: std::net::SocketAddr,
    user_args: &[OsString],
) -> Vec<OsString> {
    runtime_proxy_codex_args_with_mount_path(
        listen_addr,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH,
        user_args,
    )
}

pub(crate) fn runtime_proxy_codex_args_with_mount_path(
    listen_addr: std::net::SocketAddr,
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
