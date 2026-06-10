use super::*;

#[test]
fn version_is_newer_compares_semver_like_versions() {
    assert!(version_is_newer("0.2.47", "0.2.46"));
    assert!(version_is_newer("1.0.0", "0.9.9"));
    assert!(!version_is_newer("0.2.46", "0.2.46"));
    assert!(!version_is_newer("0.2.45", "0.2.46"));
}

#[test]
fn prodex_update_command_prefers_npm_for_native_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "npm install -g @christiandoxa/prodex@0.2.99 or npm install -g @christiandoxa/prodex@latest"
    );
}

#[test]
fn prodex_update_command_prefers_npm_for_npm_installations() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(
        prodex_update_command_for_version("0.2.99"),
        "npm install -g @christiandoxa/prodex@0.2.99 or npm install -g @christiandoxa/prodex@latest"
    );
}

#[test]
fn current_prodex_release_source_is_npm_when_wrapped_by_npm() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "@christiandoxa/prodex");
    assert_eq!(current_prodex_release_source(), ProdexReleaseSource::Npm);
}

#[test]
fn current_prodex_release_source_defaults_to_npm() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    assert_eq!(current_prodex_release_source(), ProdexReleaseSource::Npm);
}

#[test]
fn cached_update_version_is_scoped_to_release_source() {
    let now = Local::now().timestamp();
    assert!(!should_use_cached_update_version(
        ProdexReleaseSource::CratesIo,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
    assert!(should_use_cached_update_version(
        ProdexReleaseSource::Npm,
        "0.2.99",
        now,
        ProdexReleaseSource::Npm,
        current_prodex_version(),
        now
    ));
}

#[test]
fn update_notice_is_suppressed_for_machine_output_modes() {
    assert!(!should_emit_update_notice(&Commands::Info(InfoArgs::default())));
    assert!(!should_emit_update_notice(&Commands::Doctor(DoctorArgs {
        quota: false,
        runtime: true,
        install: false,
        repair_import_auth_journals: false,
        tail_bytes: RUNTIME_PROXY_DOCTOR_TAIL_BYTES,
        suggest_policy: false,
        json: true,
        bundle: None,
        redacted: false,
    })));
    assert!(!should_emit_update_notice(&Commands::Audit(AuditArgs {
        tail: 20,
        json: true,
        component: None,
        action: None,
        outcome: None,
    })));
    assert!(!should_emit_update_notice(&Commands::Context(
        ContextCommands::Audit(ContextAuditArgs {
            root: None,
            limit: 20,
            json: true,
        })
    )));
    assert!(!should_emit_update_notice(&Commands::Quota(QuotaArgs {
        profile: None,
        all: false,
        detail: false,
        raw: true,
        watch: false,
        once: false,
        auth: None,
        provider: None,
        base_url: None,
    })));
    assert!(!should_emit_update_notice(&Commands::Update(CodexUpdateArgs {
        codex_args: Vec::new(),
    })));
    assert!(should_emit_update_notice(&Commands::Current));
}

#[test]
fn doctor_tail_bytes_cli_defaults_and_overrides() {
    let command =
        parse_cli_command_from(["prodex", "doctor", "--runtime"]).expect("doctor should parse");
    let Commands::Doctor(args) = command else {
        panic!("expected doctor command");
    };
    assert_eq!(args.tail_bytes, RUNTIME_PROXY_DOCTOR_TAIL_BYTES);
    assert!(!args.repair_import_auth_journals);
    assert!(!args.suggest_policy);
    assert!(args.bundle.is_none());
    assert!(!args.redacted);

    let command = parse_cli_command_from([
        "prodex",
        "doctor",
        "--runtime",
        "--repair-import-auth-journals",
        "--suggest-policy",
        "--tail-bytes",
        "42",
    ])
    .expect("doctor tail override should parse");
    let Commands::Doctor(args) = command else {
        panic!("expected doctor command");
    };
    assert_eq!(args.tail_bytes, 42);
    assert!(args.repair_import_auth_journals);
    assert!(args.suggest_policy);
    assert!(
        parse_cli_command_from(["prodex", "doctor", "--suggest-policy"]).is_err(),
        "policy suggestions require --runtime"
    );
    assert!(
        parse_cli_command_from(["prodex", "doctor", "--bundle"]).is_err(),
        "doctor bundle requires --redacted"
    );
    let command = parse_cli_command_from(["prodex", "doctor", "--bundle", "--redacted"])
        .expect("redacted doctor bundle should parse");
    let Commands::Doctor(args) = command else {
        panic!("expected doctor command");
    };
    assert_eq!(args.bundle.as_deref(), Some(std::path::Path::new("-")));
    assert!(args.redacted);
}

#[test]
fn doctor_redaction_tolerates_codex_139_editor_and_pager_environment_fields() {
    let mut value = serde_json::json!({
        "codex_doctor": {
            "environment": {
                "editor": {
                    "name": "vim",
                    "raw_value": "EDITOR=vim"
                },
                "pager": {
                    "name": "less",
                    "raw_value": "PAGER=less"
                },
                "visual": "/usr/bin/code --wait"
            }
        }
    });

    doctor_redact_json_value(&mut value);

    assert_eq!(value["codex_doctor"]["environment"]["editor"]["name"], "vim");
    assert_eq!(value["codex_doctor"]["environment"]["pager"]["name"], "less");
    assert_eq!(
        value["codex_doctor"]["environment"]["editor"]["raw_value"],
        "<redacted>"
    );
    assert_eq!(
        value["codex_doctor"]["environment"]["pager"]["raw_value"],
        "<redacted>"
    );
}

#[test]
fn runtime_doctor_summary_uses_configured_tail_bytes() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex-home");
    let log_dir = temp_dir.path.join("logs");
    fs::create_dir_all(&prodex_home).expect("prodex home should be created");
    fs::create_dir_all(&log_dir).expect("runtime log dir should be created");
    let _prodex_home_guard = TestEnvVarGuard::set("PRODEX_HOME", prodex_home.to_str().unwrap());
    let _runtime_log_guard =
        TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_DIR", log_dir.to_str().unwrap());
    let log_path = log_dir.join(format!("{RUNTIME_PROXY_LOG_FILE_PREFIX}-tail-test.log"));
    let early_marker = "[2026-04-28T00:00:00Z] runtime_proxy_queue_overloaded lane=responses\n";
    let filler = "x".repeat(RUNTIME_PROXY_DOCTOR_TAIL_BYTES + 64);
    let late_marker = "[2026-04-28T00:00:01Z] first_local_chunk route=responses\n";
    let log_text = format!("{early_marker}{filler}\n{late_marker}");
    fs::write(&log_path, &log_text).expect("runtime log should be written");
    fs::write(
        runtime_proxy_latest_log_pointer_path(),
        format!("{}\n", log_path.display()),
    )
    .expect("runtime latest pointer should be written");

    let default_summary =
        collect_runtime_doctor_summary_with_tail_bytes(RUNTIME_PROXY_DOCTOR_TAIL_BYTES);
    assert_eq!(
        runtime_doctor_marker_count(&default_summary, "runtime_proxy_queue_overloaded"),
        0
    );
    assert_eq!(
        runtime_doctor_marker_count(&default_summary, "first_local_chunk"),
        1
    );

    let override_summary = collect_runtime_doctor_summary_with_tail_bytes(log_text.len());
    assert_eq!(
        runtime_doctor_marker_count(&override_summary, "runtime_proxy_queue_overloaded"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&override_summary, "first_local_chunk"),
        1
    );
}

#[test]
fn update_check_cache_ttl_is_short_when_cached_version_matches_current() {
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.47", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.46", "0.2.47"),
        UPDATE_CHECK_STALE_CURRENT_TTL_SECONDS
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.2.48", "0.2.47"),
        UPDATE_CHECK_CACHE_TTL_SECONDS
    );
}

#[test]
fn format_info_prodex_version_reports_up_to_date_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "Npm",
            "latest_version": current_prodex_version(),
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (up to date)", current_prodex_version())
    );
}

#[test]
fn format_info_prodex_version_reports_available_update_from_cache() {
    let _npm_name_guard = TestEnvVarGuard::set("npm_package_name", "");
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(
        update_check_cache_file_path(&paths),
        serde_json::to_string_pretty(&serde_json::json!({
            "source": "Npm",
            "latest_version": "99.0.0",
            "checked_at": Local::now().timestamp(),
        }))
        .expect("update cache json should serialize"),
    )
    .expect("update cache should save");

    assert_eq!(
        format_info_prodex_version(&paths).expect("version summary should render"),
        format!("{} (update available: 99.0.0)", current_prodex_version())
    );
}

#[test]
fn normalize_run_codex_args_rewrites_session_id_to_resume() {
    let args = vec![
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("continue from here"),
    ];
    assert_eq!(
        normalize_run_codex_args(&args),
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("continue from here"),
        ]
    );
}

#[test]
fn explicit_exec_resume_output_schema_survives_resume_normalization() {
    let args = vec![
        OsString::from("exec"),
        OsString::from("resume"),
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("--output-schema"),
        OsString::from("schema.json"),
        OsString::from("return json"),
    ];

    assert_eq!(normalize_run_codex_args(&args), args);

    let (launch_args, include_code_review) = prepare_codex_launch_args(&args, false);
    assert_eq!(launch_args, args);
    assert!(!include_code_review);
}

#[test]
fn prepare_codex_launch_args_preserves_review_detection_after_normalization() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn goal_slash_args_survive_default_run_and_resume_normalization() {
    let command = parse_cli_command_from([
        "prodex",
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "/goal",
        "resume saved objective",
    ])
    .expect("bare resume goal invocation should parse as run command");
    let Commands::Run(run_args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        run_args.codex_args,
        vec![
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("/goal"),
            OsString::from("resume saved objective"),
        ],
    );

    let (launch_args, include_code_review) = prepare_codex_launch_args(&run_args.codex_args, false);
    assert_eq!(
        launch_args,
        vec![
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("/goal"),
            OsString::from("resume saved objective"),
        ],
    );
    assert!(!include_code_review);
}
