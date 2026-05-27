use super::*;

#[test]
fn runtime_proxy_codex_args_preserve_user_overrides_after_proxy_overrides() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("service_tier=null"),
            OsString::from("--config=notice.fast_default_opt_out=true"),
            OsString::from("hello"),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert_eq!(args[0], "-c");
    assert_eq!(
        args[1],
        "chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""
    );
    assert_eq!(args[2], "-c");
    assert_eq!(
        args[3],
        format!(
            "openai_base_url=\"http://127.0.0.1:4455{}\"",
            runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH
        )
    );
    assert_eq!(
        &args[4..],
        [
            "exec",
            "-c",
            "service_tier=null",
            "--config=notice.fast_default_opt_out=true",
            "hello"
        ]
    );
}

#[test]
fn runtime_proxy_passthrough_args_rewrite_local_provider_base_url() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: Some("prodex-local"),
        }),
        &[
            OsString::from("mem"),
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-local\""),
            OsString::from("-c"),
            OsString::from("model_providers.prodex-local.base_url=\"http://127.0.0.1:8131/v1\""),
            OsString::from("exec"),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert_eq!(
        args,
        vec![
            "mem".to_string(),
            "-c".to_string(),
            "model_provider=\"prodex-local\"".to_string(),
            "-c".to_string(),
            "model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\"".to_string(),
            "exec".to_string(),
        ]
    );
}

#[test]
fn prepare_codex_launch_args_extracts_full_access_and_normalizes_resume() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("--full-access"),
            OsString::from("review"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn prepare_codex_launch_args_extracts_prodex_full_access_passthrough_marker() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("exec"),
            OsString::from("--full-access"),
            OsString::from("review"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn prepare_codex_launch_args_full_access_keeps_resume_normalization_and_review_detection() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ],
        true,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn prepare_codex_launch_args_rewrites_legacy_profile_v2_flag_for_codex_0134() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("exec"),
            OsString::from("--profile-v2"),
            OsString::from("bedrock"),
            OsString::from("--profile-v2=local"),
            OsString::from("hello"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("exec"),
            OsString::from("--profile"),
            OsString::from("bedrock"),
            OsString::from("--profile=local"),
            OsString::from("hello"),
        ]
    );
    assert!(!include_code_review);
}
