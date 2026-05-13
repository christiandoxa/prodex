#[test]
fn run_shares_resume_history_across_managed_profiles() {
    let fixture = setup_fixture();
    let seeded_session_dir = fixture.main_home.join("sessions/2026/03");
    fs::create_dir_all(&seeded_session_dir).expect("failed to create seeded session dir");
    fs::write(fixture.main_home.join("history.jsonl"), "seed-main\n")
        .expect("failed to seed history");
    fs::write(seeded_session_dir.join("seed.json"), "{\"seed\":true}\n")
        .expect("failed to seed session");

    let first_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_SESSION_MARKER", "main-run")],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    let second_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
        &[("TEST_SESSION_MARKER", "second-run")],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    let main_history = fs::read_to_string(fixture.main_home.join("history.jsonl"))
        .expect("failed to read main history");
    let second_history = fs::read_to_string(fixture.second_home.join("history.jsonl"))
        .expect("failed to read second history");
    assert_eq!(main_history, second_history);
    assert!(main_history.contains("seed-main"));
    assert!(main_history.contains("main-run"));
    assert!(main_history.contains("second-run"));

    assert!(
        fixture
            .main_home
            .join("sessions/2026/03/seed.json")
            .is_file()
    );
    assert!(
        fixture
            .second_home
            .join("sessions/2026/03/seed.json")
            .is_file()
    );
    assert!(fixture.main_home.join("sessions/main-run.json").is_file());
    assert!(fixture.second_home.join("sessions/main-run.json").is_file());
    assert!(fixture.main_home.join("sessions/second-run.json").is_file());
    assert!(
        fixture
            .second_home
            .join("sessions/second-run.json")
            .is_file()
    );

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("history.jsonl"))
                .expect("failed to read main history link"),
            fixture.shared_codex_home.join("history.jsonl")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("history.jsonl"))
                .expect("failed to read second history link"),
            fixture.shared_codex_home.join("history.jsonl")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("sessions"))
                .expect("failed to read main sessions link"),
            fixture.shared_codex_home.join("sessions")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("sessions"))
                .expect("failed to read second sessions link"),
            fixture.shared_codex_home.join("sessions")
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("history.jsonl"))
                .expect("failed to inspect main history")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("history.jsonl"))
                .expect("failed to inspect second history")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("sessions"))
                .expect("failed to inspect main sessions")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("sessions"))
                .expect("failed to inspect second sessions")
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn run_shares_housekeeping_memories_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::create_dir_all(fixture.main_home.join("memories")).expect("failed to create memories dir");
    fs::write(
        fixture.main_home.join("memories/seed-memory.json"),
        "{\"seed\":true}\n",
    )
    .expect("failed to seed memory");

    let first_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_MEMORY_MARKER", "main-memory")],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    let second_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
        &[("TEST_MEMORY_MARKER", "second-memory")],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    for home in [&fixture.main_home, &fixture.second_home] {
        assert!(home.join("memories/seed-memory.json").is_file());
        assert!(home.join("memories/main-memory.json").is_file());
        assert!(home.join("memories/second-memory.json").is_file());
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("memories"))
                .expect("failed to read main memories link"),
            fixture.shared_codex_home.join("memories")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("memories"))
                .expect("failed to read second memories link"),
            fixture.shared_codex_home.join("memories")
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("memories"))
                .expect("failed to inspect main memories")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("memories"))
                .expect("failed to inspect second memories")
                .file_type()
                .is_symlink()
        );
    }
}

