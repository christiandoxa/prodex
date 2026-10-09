use super::*;

#[test]
fn runtime_launch_cli_model_accepts_inline_and_short_forms() {
    assert_eq!(
        runtime_launch_cli_model(&[OsString::from("--model=gpt-5.6")]).as_deref(),
        Some("gpt-5.6")
    );
    assert_eq!(
        runtime_launch_cli_model(&[OsString::from("-mgpt-5.6")]).as_deref(),
        Some("gpt-5.6")
    );
}

#[test]
fn runtime_proxy_codex_args_keep_workspace_bootstrap_codex_owned() {
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

    assert!(!args.iter().any(|arg| {
        arg.starts_with("chatgpt_base_url=") || arg.starts_with("openai_base_url=")
    }));
    assert!(
        args.iter()
            .any(|arg| { arg == "model_provider=\"prodex-openai-governed-http\"" })
    );
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.base_url=\"http://127.0.0.1:4455/backend-api/prodex\""
    }));
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.requires_openai_auth=true"
    }));
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.supports_websockets=true"
    }));
    assert!(args.iter().any(|arg| arg == "service_tier=null"));
    assert!(
        args.iter()
            .any(|arg| arg == "--config=notice.fast_default_opt_out=true")
    );
    assert_eq!(args.last().map(String::as_str), Some("hello"));
}

#[test]
fn runtime_proxy_codex_args_preserve_user_https_chatgpt_bootstrap() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[
            OsString::from("-c"),
            OsString::from("chatgpt_base_url=\"https://workspace.example\""),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert!(
        args.iter()
            .any(|arg| { arg == "chatgpt_base_url=\"https://workspace.example\"" })
    );
    assert!(
        !args
            .iter()
            .any(|arg| { arg == "chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\"" })
    );
}

#[test]
fn runtime_proxy_passthrough_args_rewrite_local_provider_base_url() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: Some("prodex-local"),
            force_http_responses: false,
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
            force_http_responses: false,
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
            force_http_responses: false,
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
fn runtime_proxy_child_args_keep_thread_source_and_its_value_adjacent() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: Some("prodex-local"),
            force_http_responses: false,
            realtime_ws_base_url: None,
            realtime_ws_model: None,
        }),
        &[
            OsString::from("--thread-source"),
            OsString::from("automated_review"),
            OsString::from("exec"),
            OsString::from("fork"),
            OsString::from("thread-123"),
            OsString::from("-"),
        ],
    );

    assert_eq!(
        scope_codex_exec_config_args(&args),
        vec![
            OsString::from("--thread-source"),
            OsString::from("automated_review"),
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\""),
            OsString::from("fork"),
            OsString::from("thread-123"),
            OsString::from("-"),
        ]
    );
}

#[test]
fn runtime_proxy_passthrough_args_force_governed_responses_to_http() {
    let args = runtime_proxy_codex_passthrough_args(
        Some(RuntimeProxyCodexEndpoint {
            listen_addr: "127.0.0.1:4455".parse().expect("socket addr"),
            openai_mount_path: "/v1",
            local_model_provider_id: None,
            force_http_responses: true,
            realtime_ws_base_url: None,
            realtime_ws_model: None,
        }),
        &[
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("model_provider=\"openai\""),
            OsString::from("hello"),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    let governed_provider = args
        .iter()
        .rposition(|arg| arg == "model_provider=\"prodex-openai-governed-http\"")
        .expect("governed provider override should exist");
    let user_provider = args
        .iter()
        .position(|arg| arg == "model_provider=\"openai\"")
        .expect("user provider override should remain visible");
    assert!(governed_provider > user_provider);
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.supports_websockets=false"
    }));
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.supports_standalone_web_search=true"
    }));
    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.base_url=\"http://127.0.0.1:4455/v1\""
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

#[path = "args_resume.rs"]
mod args_resume;
