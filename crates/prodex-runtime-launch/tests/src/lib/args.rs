use super::*;
use runtime_proxy_crate as runtime_proxy;

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
            realtime_ws_base_url: None,
            realtime_ws_model: None,
        }),
        &[
            OsString::from("review"),
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
            "review".to_string(),
            "-c".to_string(),
            "model_provider=\"prodex-local\"".to_string(),
            "-c".to_string(),
            "model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\"".to_string(),
            "exec".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_passthrough_args_add_realtime_sidecar_overrides() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: Some("prodex-gemini"),
            realtime_ws_base_url: Some("http://127.0.0.1:4555"),
            realtime_ws_model: Some("gemini-3.1-flash-live-preview"),
        }),
        &[
            OsString::from("-c"),
            OsString::from("experimental_realtime_ws_model=\"old\""),
            OsString::from("exec"),
            OsString::from("hello"),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert!(args.windows(2).any(|window| {
        window[0] == "-c"
            && window[1] == "experimental_realtime_ws_base_url=\"http://127.0.0.1:4555\""
    }));
    assert!(args.windows(2).any(|window| {
        window[0] == "-c"
            && window[1] == "experimental_realtime_ws_model=\"gemini-3.1-flash-live-preview\""
    }));
    assert!(!args.iter().any(|arg| {
        arg == "experimental_realtime_ws_model=\"old\""
            || arg == "--config=experimental_realtime_ws_model=\"old\""
    }));
    assert_eq!(args.last().map(String::as_str), Some("hello"));
}

#[test]
fn runtime_proxy_passthrough_args_insert_local_provider_override_before_prompt() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: Some("prodex-local"),
            realtime_ws_base_url: None,
            realtime_ws_model: None,
        }),
        &[OsString::from("exec"), OsString::from("hello")],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert!(args.windows(2).any(|window| {
        window[0] == "-c"
            && window[1] == "model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\""
    }));
    assert_eq!(args.last().map(String::as_str), Some("hello"));
}

#[test]
fn scope_codex_exec_config_args_moves_pre_exec_overrides_into_exec_scope() {
    let args = scope_codex_exec_config_args(&[
        OsString::from("-c"),
        OsString::from("model_catalog_json=\"/tmp/catalog.json\""),
        OsString::from("--dangerously-bypass-approvals-and-sandbox"),
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-gemini\""),
        OsString::from("--config=model=\"auto\""),
        OsString::from("-cmodel_context_window=1048576"),
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("-c"),
        OsString::from("model_reasoning_effort=\"high\""),
        OsString::from("hello"),
    ]);

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("model_catalog_json=\"/tmp/catalog.json\""),
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"auto\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from("--json"),
            OsString::from("-c"),
            OsString::from("model_reasoning_effort=\"high\""),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn scope_codex_exec_config_args_moves_exec_resume_overrides_into_resume_scope() {
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let args = scope_codex_exec_config_args(&[
        OsString::from("-c"),
        OsString::from("model_catalog_json=\"/tmp/catalog.json\""),
        OsString::from("--dangerously-bypass-approvals-and-sandbox"),
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-gemini\""),
        OsString::from("--config=model=\"auto\""),
        OsString::from("-cmodel_context_window=1048576"),
        OsString::from("resume"),
        OsString::from(session_id),
        OsString::from("hello"),
    ]);

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("--json"),
            OsString::from("resume"),
            OsString::from("-c"),
            OsString::from("model_catalog_json=\"/tmp/catalog.json\""),
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"auto\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from(session_id),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn scope_codex_exec_config_args_leaves_non_exec_commands_unchanged() {
    let original = vec![
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-gemini\""),
        OsString::from("resume"),
        OsString::from("--last"),
    ];

    assert_eq!(scope_codex_exec_config_args(&original), original);
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
fn prepare_codex_launch_args_treats_permissions_profile_alias_as_option_value() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("sandbox"),
            OsString::from("-P"),
            OsString::from(":workspace"),
            OsString::from("--"),
            OsString::from("echo"),
            OsString::from("ok"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("sandbox"),
            OsString::from("-P"),
            OsString::from(":workspace"),
            OsString::from("--"),
            OsString::from("echo"),
            OsString::from("ok"),
        ]
    );
    assert!(!include_code_review);
}

#[test]
fn codex_resume_last_prompt_is_not_treated_as_session_id() {
    let args = vec![
        OsString::from("resume"),
        OsString::from("--last"),
        OsString::from("continue from current context"),
    ];

    assert_eq!(codex_resume_session_id(&args), None);
    assert_eq!(normalize_run_codex_args(&args), args);
}

#[test]
fn codex_fork_last_prompt_survives_launch_normalization() {
    let args = vec![
        OsString::from("fork"),
        OsString::from("--last"),
        OsString::from("continue from current context"),
    ];

    let (normalized, include_code_review) = prepare_codex_launch_args(&args, false);

    assert_eq!(normalized, args);
    assert!(!include_code_review);
}

#[test]
fn prepare_codex_launch_args_normalizes_resume_after_provider_config_overrides() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"gemini-2.5-pro\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"gemini-2.5-pro\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
    assert!(!include_code_review);
}

#[test]
fn codex_resume_session_id_extracts_normalized_resume_target() {
    let (args, _) = prepare_codex_launch_args(
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ],
        false,
    );

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn codex_resume_session_id_extracts_explicit_exec_resume_target() {
    let args = [
        OsString::from("exec"),
        OsString::from("resume"),
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-gemini\""),
        OsString::from("--config=model=\"auto\""),
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("continue"),
    ];

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn codex_resume_session_id_extracts_target_before_prompt() {
    let args = [
        OsString::from("resume"),
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("/compact focus on auth"),
    ];

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn retarget_codex_tui_resume_args_preserves_global_options_and_replaces_prompt() {
    let args = retarget_codex_tui_resume_args(
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("initial prompt"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn codex_resume_session_id_ignores_resume_without_target() {
    assert_eq!(
        codex_resume_session_id(&[OsString::from("resume"), OsString::from("--last")]),
        None
    );
    assert_eq!(
        codex_resume_session_id(&[
            OsString::from("resume"),
            OsString::from("--last"),
            OsString::from("/compact focus on auth"),
        ]),
        None
    );
    assert_eq!(
        codex_resume_session_id(&[
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("--last"),
            OsString::from("continue from latest"),
        ]),
        None
    );
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
fn prepare_codex_launch_args_preserves_markers_after_separator() {
    let input = vec![
        OsString::from("exec"),
        OsString::from("--"),
        OsString::from("--full-access"),
        OsString::from("--dry-run"),
        OsString::from("--profile-v2=literal"),
        OsString::from("review"),
    ];

    let (args, include_code_review) = prepare_codex_launch_args(&input, false);

    assert_eq!(args, input);
    assert!(!include_code_review);
    assert_eq!(extract_prodex_dry_run_flag(&args), (false, args.clone()));
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
