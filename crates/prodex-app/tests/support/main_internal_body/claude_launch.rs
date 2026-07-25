use super::*;

#[test]
fn claude_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "claude",
        "--profile",
        "main",
        "--",
        "-p",
        "--output-format",
        "json",
        "hello",
    ])
    .expect("claude command should parse");
    let Commands::Claude(args) = command else {
        panic!("expected claude command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.claude_args,
        vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("json"),
            OsString::from("hello"),
        ]
    );
}

#[test]
fn claude_caveman_mode_extracts_prefix_and_preserves_passthrough_args() {
    let (launch_modes, claude_args) = runtime_proxy_claude_extract_launch_modes(&[
        OsString::from("caveman"),
        OsString::from("-p"),
        OsString::from("hello"),
    ]);
    assert!(launch_modes.caveman_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );

    let (launch_modes, claude_args) =
        runtime_proxy_claude_extract_launch_modes(&[OsString::from("-p"), OsString::from("hi")]);
    assert!(!launch_modes.caveman_mode);
    assert_eq!(
        claude_args,
        vec![OsString::from("-p"), OsString::from("hi")]
    );
}

#[test]
fn runtime_proxy_claude_launch_args_prepend_plugin_dirs_when_present() {
    let launch_args = runtime_proxy_claude_launch_args(
        &[OsString::from("-p"), OsString::from("hello")],
        &[PathBuf::from("/tmp/prodex-caveman-plugin")],
    );
    assert_eq!(
        launch_args,
        vec![
            OsString::from("--plugin-dir"),
            OsString::from("/tmp/prodex-caveman-plugin"),
            OsString::from("-p"),
            OsString::from("hello"),
        ]
    );

    let launch_args =
        runtime_proxy_claude_launch_args(&[OsString::from("-p"), OsString::from("hello")], &[]);
    assert_eq!(
        launch_args,
        vec![OsString::from("-p"), OsString::from("hello")]
    );
}

#[test]
fn missing_external_caveman_fails_before_claude_launch() {
    let tools = TestDir::new();
    let _env_guard = TestEnvVarGuard::set(
        prodex_optional_tools::PRODEX_OPTIMIZERS_HOME_ENV,
        tools.path.to_str().expect("temporary path should be UTF-8"),
    );
    let paths = AppPaths {
        root: tools.path.clone(),
        state_file: tools.path.join("state.json"),
        managed_profiles_root: tools.path.join("profiles"),
        shared_codex_root: tools.path.join(".codex"),
        legacy_shared_codex_root: tools.path.join("shared"),
    };

    let error = prepare_runtime_proxy_claude_caveman_plugin_dir(&paths).unwrap_err();

    assert!(error.to_string().contains("not installed"));
}

#[test]
fn prodex_overlay_does_not_install_optional_tools() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    create_codex_home_if_missing(&paths.managed_profiles_root).unwrap();
    let base_home = paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&base_home).unwrap();
    fs::write(base_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let overlay = prepare_prodex_overlay_home(&paths, &base_home).unwrap();

    assert_eq!(
        fs::read_to_string(overlay.join("config.toml")).unwrap(),
        "model = \"gpt-5\"\n"
    );
    assert!(!overlay.join(".tmp/marketplaces/prodex-caveman").exists());
    assert!(!overlay.join("plugins/cache/prodex-caveman").exists());
}

#[cfg(unix)]
#[test]
fn prepare_prodex_overlay_home_preserves_pasted_attachments_across_profile_resume() {
    let temp_dir = TestDir::new();
    let paths = AppPaths {
        root: temp_dir.path.clone(),
        state_file: temp_dir.path.join("state.json"),
        managed_profiles_root: temp_dir.path.join("profiles"),
        shared_codex_root: temp_dir.path.join(".codex"),
        legacy_shared_codex_root: temp_dir.path.join("shared"),
    };
    let first_profile_home = paths.managed_profiles_root.join("first");
    let second_profile_home = paths.managed_profiles_root.join("second");

    prodex_shared_codex_fs::prepare_managed_codex_home(&paths, &first_profile_home)
        .expect("first managed profile should prepare");
    let first_overlay = prepare_prodex_overlay_home(&paths, &first_profile_home)
        .expect("first Prodex overlay should prepare");

    let attachment_id = "bbbbbbbb-cccc-4ddd-8eee-ffffffffffff";
    let overlay_pasted_text = first_overlay
        .join("attachments")
        .join(attachment_id)
        .join("pasted-text-1.txt");
    let overlay_pasted_image = first_overlay
        .join("attachments")
        .join(attachment_id)
        .join("image-1.png");
    fs::create_dir_all(overlay_pasted_text.parent().expect("attachment parent"))
        .expect("attachment parent should create");
    fs::write(&overlay_pasted_text, b"pasted text from first overlay")
        .expect("overlay pasted text should write");
    fs::write(&overlay_pasted_image, b"pasted image from first overlay")
        .expect("overlay pasted image should write");

    let session_file = first_overlay.join("sessions/2026/06/26/rollout-session.jsonl");
    fs::create_dir_all(session_file.parent().expect("session parent"))
        .expect("session parent should create");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-26T08:00:00Z","type":"response_item","payload":{{"text":"Pasted Content: {} and image {}"}}}}"#,
            overlay_pasted_text.display(),
            overlay_pasted_image.display()
        ),
    )
    .expect("session should write through overlay symlink");
    let goals_db = paths.shared_codex_root.join("goals_1.sqlite");
    let conn = rusqlite::Connection::open(&goals_db).expect("goals db should open");
    conn.execute_batch(
        r#"
        CREATE TABLE thread_goals (
            thread_id TEXT PRIMARY KEY NOT NULL,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL,
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        "#,
    )
    .expect("goals schema should create");
    conn.execute(
        "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', ?2, 'paused', 1, 1)",
        rusqlite::params![
            "thread-1",
            format!(
                "pasted text file: {}. image file: {}",
                overlay_pasted_text.display(),
                overlay_pasted_image.display()
            )
        ],
    )
    .expect("goal row should insert");
    drop(conn);

    prodex_shared_codex_fs::maintain_managed_codex_sessions(&paths)
        .expect("post-exit maintenance should stabilize attachment paths");
    prodex_shared_codex_fs::prepare_managed_codex_home(&paths, &second_profile_home)
        .expect("second managed profile should prepare");
    let second_overlay = prepare_prodex_overlay_home(&paths, &second_profile_home)
        .expect("second Prodex overlay should prepare");

    let shared_pasted_text = paths
        .shared_codex_root
        .join("attachments")
        .join(attachment_id)
        .join("pasted-text-1.txt");
    let shared_pasted_image = paths
        .shared_codex_root
        .join("attachments")
        .join(attachment_id)
        .join("image-1.png");
    assert_eq!(
        fs::read(&shared_pasted_text).expect("shared pasted text should remain readable"),
        b"pasted text from first overlay"
    );
    assert_eq!(
        fs::read(
            second_overlay
                .join("attachments")
                .join(attachment_id)
                .join("image-1.png")
        )
        .expect("resumed overlay should see pasted image"),
        b"pasted image from first overlay"
    );
    assert_eq!(
        fs::read_link(second_profile_home.join("attachments"))
            .expect("second profile attachments should be shared"),
        paths.shared_codex_root.join("attachments")
    );
    assert_eq!(
        fs::read_link(second_overlay.join("attachments"))
            .expect("second overlay attachments should point at second profile"),
        second_profile_home.join("attachments")
    );

    let shared_session = paths
        .shared_codex_root
        .join("sessions/2026/06/26/rollout-session.jsonl");
    let rewritten = fs::read_to_string(&shared_session).expect("shared session should read");
    assert!(
        rewritten.contains(&shared_pasted_text.display().to_string()),
        "resume history should point at stable shared pasted text path: {rewritten}"
    );
    assert!(
        rewritten.contains(&shared_pasted_image.display().to_string()),
        "resume history should point at stable shared pasted image path: {rewritten}"
    );
    assert!(
        !rewritten.contains(&first_overlay.display().to_string()),
        "resume history must not retain first overlay path: {rewritten}"
    );

    let conn = rusqlite::Connection::open(&goals_db).expect("goals db should reopen");
    let goal_objective: String = conn
        .query_row(
            "SELECT objective FROM thread_goals WHERE thread_id = 'thread-1'",
            [],
            |row| row.get(0),
        )
        .expect("goal objective should read");
    assert!(
        goal_objective.contains(&shared_pasted_text.display().to_string()),
        "goal objective should point at shared pasted text: {goal_objective}"
    );
    assert!(
        goal_objective.contains(&shared_pasted_image.display().to_string()),
        "goal objective should point at shared pasted image: {goal_objective}"
    );
    assert!(
        !goal_objective.contains(&first_overlay.display().to_string()),
        "goal objective must not retain first overlay path: {goal_objective}"
    );
}
