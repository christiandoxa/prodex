#[test]
fn run_shares_native_codex_behavior_state_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::write(
        fixture.main_home.join("config.toml"),
        "model = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n",
    )
    .expect("failed to seed config");
    fs::write(
        fixture.main_home.join("AGENTS.md"),
        "# Main profile global instructions\n",
    )
    .expect("failed to seed AGENTS.md");
    fs::write(
        fixture.main_home.join("AGENTS.override.md"),
        "# Main profile global override instructions\n",
    )
    .expect("failed to seed AGENTS.override.md");
    fs::create_dir_all(fixture.main_home.join("rules")).expect("failed to create main rules dir");
    fs::write(
        fixture.main_home.join("rules/default.rules"),
        "main-rule = true\n",
    )
    .expect("failed to seed main rule");
    fs::create_dir_all(fixture.main_home.join("skills/main-skill"))
        .expect("failed to create main skill dir");
    fs::write(
        fixture.main_home.join("skills/main-skill/SKILL.md"),
        "# Main Skill\n",
    )
    .expect("failed to seed main skill");
    fs::create_dir_all(fixture.main_home.join("agents")).expect("failed to create main agents dir");
    fs::write(
        fixture.main_home.join("agents/reviewer.md"),
        "# Reviewer Agent\n",
    )
    .expect("failed to seed main agent");

    let first_output = run_prodex(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    fs::create_dir_all(fixture.second_home.join("rules"))
        .expect("failed to create second rules dir");
    fs::write(
        fixture.second_home.join("rules/team.rules"),
        "second-rule = true\n",
    )
    .expect("failed to seed second rule");
    fs::create_dir_all(fixture.second_home.join("skills/second-skill"))
        .expect("failed to create second skill dir");
    fs::write(
        fixture.second_home.join("skills/second-skill/SKILL.md"),
        "# Second Skill\n",
    )
    .expect("failed to seed second skill");
    fs::create_dir_all(fixture.second_home.join("agents"))
        .expect("failed to create second agents dir");
    fs::write(
        fixture.second_home.join("agents/triage.md"),
        "# Triage Agent\n",
    )
    .expect("failed to seed second agent");

    let second_output = run_prodex(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    for home in [&fixture.main_home, &fixture.second_home] {
        let config = fs::read_to_string(home.join("config.toml"))
            .expect("shared config.toml should be readable");
        assert!(config.contains("model_reasoning_effort = \"xhigh\""));
        let agents = fs::read_to_string(home.join("AGENTS.md"))
            .expect("shared AGENTS.md should be readable");
        assert!(agents.contains("Main profile global instructions"));
        let agents_override = fs::read_to_string(home.join("AGENTS.override.md"))
            .expect("shared AGENTS.override.md should be readable");
        assert!(agents_override.contains("Main profile global override instructions"));
        assert!(home.join("rules/default.rules").is_file());
        assert!(home.join("rules/team.rules").is_file());
        assert!(home.join("skills/main-skill/SKILL.md").is_file());
        assert!(home.join("skills/second-skill/SKILL.md").is_file());
        assert!(home.join("agents/reviewer.md").is_file());
        assert!(home.join("agents/triage.md").is_file());
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("config.toml"))
                .expect("failed to read main config link"),
            fixture.shared_codex_home.join("config.toml")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("config.toml"))
                .expect("failed to read second config link"),
            fixture.shared_codex_home.join("config.toml")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("AGENTS.md"))
                .expect("failed to read main AGENTS.md link"),
            fixture.shared_codex_home.join("AGENTS.md")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("AGENTS.md"))
                .expect("failed to read second AGENTS.md link"),
            fixture.shared_codex_home.join("AGENTS.md")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("AGENTS.override.md"))
                .expect("failed to read main AGENTS.override.md link"),
            fixture.shared_codex_home.join("AGENTS.override.md")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("AGENTS.override.md"))
                .expect("failed to read second AGENTS.override.md link"),
            fixture.shared_codex_home.join("AGENTS.override.md")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("rules")).expect("failed to read main rules link"),
            fixture.shared_codex_home.join("rules")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("rules"))
                .expect("failed to read second rules link"),
            fixture.shared_codex_home.join("rules")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("skills"))
                .expect("failed to read main skills link"),
            fixture.shared_codex_home.join("skills")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("skills"))
                .expect("failed to read second skills link"),
            fixture.shared_codex_home.join("skills")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("agents"))
                .expect("failed to read main agents link"),
            fixture.shared_codex_home.join("agents")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("agents"))
                .expect("failed to read second agents link"),
            fixture.shared_codex_home.join("agents")
        );
    }
}
