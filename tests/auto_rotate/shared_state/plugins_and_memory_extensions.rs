#[test]
fn run_shares_codex_plugin_and_memory_extension_state_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::write(
        fixture.main_home.join("config.toml"),
        r#"[features]
plugins = true

[marketplaces.debug]
source_type = "git"
source = "https://github.com/example/debug-marketplace.git"
last_updated = "2026-04-16T00:00:00Z"

[plugins."sample-plugin@debug"]
enabled = true
"#,
    )
    .expect("failed to seed marketplace config");

    fs::create_dir_all(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/.agents/plugins"),
    )
    .expect("failed to create main marketplace manifest dir");
    write_json(
        &fixture
            .main_home
            .join(".tmp/marketplaces/debug/.agents/plugins/marketplace.json"),
        &json!({
            "name": "debug",
            "plugins": [
                {
                    "name": "sample-plugin",
                    "source": {
                        "type": "local",
                        "path": "./plugins/sample-plugin"
                    },
                    "policy": {
                        "installation": "AVAILABLE",
                        "authentication": "ON_INSTALL"
                    }
                }
            ]
        }),
    );
    fs::create_dir_all(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/.codex-plugin"),
    )
    .expect("failed to create main marketplace plugin dir");
    write_json(
        &fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "local"
        }),
    );
    fs::write(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/marketplace-main.txt"),
        "main marketplace marker\n",
    )
    .expect("failed to seed main marketplace marker");

    fs::create_dir_all(
        fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/.codex-plugin"),
    )
    .expect("failed to create main plugin cache dir");
    write_json(
        &fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "local"
        }),
    );
    fs::write(
        fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/plugin-main.txt"),
        "main plugin marker\n",
    )
    .expect("failed to seed main plugin marker");

    fs::create_dir_all(
        fixture
            .main_home
            .join(".tmp/plugins/app-server/debug/sample-plugin"),
    )
    .expect("failed to create main app-server plugin cache dir");
    fs::write(
        fixture
            .main_home
            .join(".tmp/plugins/app-server/debug/sample-plugin/plugin-main.txt"),
        "main app-server plugin marker\n",
    )
    .expect("failed to seed main app-server plugin marker");
    fs::write(
        fixture.main_home.join(".tmp/plugins.sha"),
        "main-plugin-sha\n",
    )
    .expect("failed to seed plugins sha");
    write_json(
        &fixture.main_home.join(".tmp/known_marketplaces.json"),
        &json!({
            "debug": {
                "source": "https://github.com/example/debug-marketplace.git"
            }
        }),
    );
    fs::write(
        fixture
            .main_home
            .join(".tmp/app-server-remote-plugin-sync-v1"),
        "synced\n",
    )
    .expect("failed to seed app-server remote plugin sync marker");

    fs::create_dir_all(fixture.main_home.join("memories_extensions/team/resources"))
        .expect("failed to create main memories extension dir");
    fs::write(
        fixture
            .main_home
            .join("memories_extensions/team/instructions.md"),
        "# Team memory extension\n",
    )
    .expect("failed to seed extension instructions");
    fs::write(
        fixture
            .main_home
            .join("memories_extensions/team/resources/main.txt"),
        "main extension marker\n",
    )
    .expect("failed to seed main extension marker");

    let first_output = run_prodex_with_direct_provider(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    fs::create_dir_all(
        fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/.codex-plugin"),
    )
    .expect("failed to create second plugin cache dir");
    write_json(
        &fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "1.2.3"
        }),
    );
    fs::write(
        fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/plugin-second.txt"),
        "second plugin marker\n",
    )
    .expect("failed to seed second plugin marker");

    fs::create_dir_all(
        fixture
            .second_home
            .join(".tmp/plugins/app-server/debug/sample-plugin"),
    )
    .expect("failed to create second app-server plugin cache dir");
    fs::write(
        fixture
            .second_home
            .join(".tmp/plugins/app-server/debug/sample-plugin/plugin-second.txt"),
        "second app-server plugin marker\n",
    )
    .expect("failed to seed second app-server plugin marker");

    fs::create_dir_all(
        fixture
            .second_home
            .join("memories_extensions/team/resources"),
    )
    .expect("failed to create second memories extension dir");
    fs::write(
        fixture
            .second_home
            .join("memories_extensions/team/resources/second.txt"),
        "second extension marker\n",
    )
    .expect("failed to seed second extension marker");

    let second_output = run_prodex_with_direct_provider(
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
        assert!(config.contains("[marketplaces.debug]"));
        assert!(config.contains("[plugins.\"sample-plugin@debug\"]"));
        assert!(
            home.join(".tmp/marketplaces/debug/.agents/plugins/marketplace.json")
                .is_file()
        );
        assert!(
            home.join(".tmp/marketplaces/debug/plugins/sample-plugin/marketplace-main.txt")
                .is_file()
        );
        assert!(
            home.join("plugins/cache/debug/sample-plugin/local/plugin-main.txt")
                .is_file()
        );
        assert!(
            home.join("plugins/cache/debug/sample-plugin/1.2.3/plugin-second.txt")
                .is_file()
        );
        assert!(
            home.join(".tmp/plugins/app-server/debug/sample-plugin/plugin-main.txt")
                .is_file()
        );
        assert!(
            home.join(".tmp/plugins/app-server/debug/sample-plugin/plugin-second.txt")
                .is_file()
        );
        let plugins_sha = fs::read_to_string(home.join(".tmp/plugins.sha"))
            .expect("plugins sha should be readable");
        assert!(plugins_sha.contains("main-plugin-sha"));
        let known_marketplaces = fs::read_to_string(home.join(".tmp/known_marketplaces.json"))
            .expect("known marketplaces should be readable");
        assert!(known_marketplaces.contains("debug-marketplace.git"));
        let app_server_sync =
            fs::read_to_string(home.join(".tmp/app-server-remote-plugin-sync-v1"))
                .expect("app-server remote plugin sync marker should be readable");
        assert!(app_server_sync.contains("synced"));
        assert!(
            home.join("memories_extensions/team/instructions.md")
                .is_file()
        );
        assert!(
            home.join("memories_extensions/team/resources/main.txt")
                .is_file()
        );
        assert!(
            home.join("memories_extensions/team/resources/second.txt")
                .is_file()
        );
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("plugins"))
                .expect("failed to read main plugins link"),
            fixture.shared_codex_home.join("plugins")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("plugins"))
                .expect("failed to read second plugins link"),
            fixture.shared_codex_home.join("plugins")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("memories_extensions"))
                .expect("failed to read main memories_extensions link"),
            fixture.shared_codex_home.join("memories_extensions")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("memories_extensions"))
                .expect("failed to read second memories_extensions link"),
            fixture.shared_codex_home.join("memories_extensions")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join(".tmp/marketplaces"))
                .expect("failed to read main marketplaces link"),
            fixture.shared_codex_home.join(".tmp/marketplaces")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join(".tmp/marketplaces"))
                .expect("failed to read second marketplaces link"),
            fixture.shared_codex_home.join(".tmp/marketplaces")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join(".tmp/plugins"))
                .expect("failed to read main .tmp plugins link"),
            fixture.shared_codex_home.join(".tmp/plugins")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join(".tmp/plugins"))
                .expect("failed to read second .tmp plugins link"),
            fixture.shared_codex_home.join(".tmp/plugins")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join(".tmp/plugins.sha"))
                .expect("failed to read main plugins sha link"),
            fixture.shared_codex_home.join(".tmp/plugins.sha")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join(".tmp/plugins.sha"))
                .expect("failed to read second plugins sha link"),
            fixture.shared_codex_home.join(".tmp/plugins.sha")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join(".tmp/known_marketplaces.json"))
                .expect("failed to read main known marketplaces link"),
            fixture
                .shared_codex_home
                .join(".tmp/known_marketplaces.json")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join(".tmp/known_marketplaces.json"))
                .expect("failed to read second known marketplaces link"),
            fixture
                .shared_codex_home
                .join(".tmp/known_marketplaces.json")
        );
        assert_eq!(
            fs::read_link(
                fixture
                    .main_home
                    .join(".tmp/app-server-remote-plugin-sync-v1")
            )
            .expect("failed to read main app-server sync link"),
            fixture
                .shared_codex_home
                .join(".tmp/app-server-remote-plugin-sync-v1")
        );
        assert_eq!(
            fs::read_link(
                fixture
                    .second_home
                    .join(".tmp/app-server-remote-plugin-sync-v1")
            )
            .expect("failed to read second app-server sync link"),
            fixture
                .shared_codex_home
                .join(".tmp/app-server-remote-plugin-sync-v1")
        );
    }
}
