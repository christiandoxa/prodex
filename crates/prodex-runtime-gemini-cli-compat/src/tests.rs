use super::*;
use crate::{gemini_settings_source_paths_for_config_home, parse_gemini_settings_json};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .canonicalize()
        .expect("temp dir should resolve")
        .join(format!("prodex-gemini-cli-compat-{name}-{stamp}"))
}

#[cfg(unix)]
#[test]
fn gemini_settings_sources_ignore_symlinked_settings_files() {
    let root = temp_dir("settings-symlink");
    let workspace = root.join("workspace");
    let outside = root.join("outside");
    fs::create_dir_all(workspace.join(".gemini")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("settings.json"),
        serde_json::json!({
            "mcpServers": {
                "leaked": {"command": "echo"}
            }
        })
        .to_string(),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.join("settings.json"),
        workspace.join(".gemini").join("settings.json"),
    )
    .unwrap();

    let sources = gemini_settings_sources(Some(&workspace));

    assert!(
        sources
            .iter()
            .all(|source| source.directory != workspace.join(".gemini"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_settings_sources_ignore_oversized_settings_files() {
    let root = temp_dir("settings-oversized");
    let workspace = root.join("workspace");
    let settings_dir = workspace.join(".gemini");
    fs::create_dir_all(&settings_dir).unwrap();
    fs::write(
        settings_dir.join("settings.json"),
        vec![b'{'; 512 * 1024 + 1],
    )
    .unwrap();

    let sources = gemini_settings_sources(Some(&workspace));

    assert!(
        sources
            .iter()
            .all(|source| source.directory != settings_dir)
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn gemini_cli_compat_ignores_symlinked_extension_skill_dirs() {
    let root = temp_dir("skill-source-symlink");
    let extension_dir = root.join("extension");
    let outside = root.join("outside");
    fs::create_dir_all(extension_dir.join("skills")).unwrap();
    fs::create_dir_all(outside.join("review")).unwrap();
    fs::write(outside.join("review").join("SKILL.md"), "outside").unwrap();
    std::os::unix::fs::symlink(
        outside.join("review"),
        extension_dir.join("skills").join("review"),
    )
    .unwrap();
    let extension = GeminiExtension {
        directory: extension_dir,
        name: "workspace".to_string(),
        value: serde_json::json!({}),
    };

    assert!(extension_skill_dirs(&extension).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn gemini_cli_compat_does_not_cleanup_symlinked_generated_skill_dirs() {
    let root = temp_dir("skill-cleanup-symlink");
    let codex_home = root.join("codex");
    let skills_root = codex_home.join(".agents").join("skills");
    let outside = root.join("outside").join("generated-skill");
    fs::create_dir_all(&skills_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join(GENERATED_SKILL_MARKER_FILE), "outside").unwrap();
    let linked = skills_root.join("gemini-linked");
    std::os::unix::fs::symlink(&outside, &linked).unwrap();

    write_gemini_skills(&codex_home, &[]).unwrap();

    assert!(
        fs::symlink_metadata(&linked)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(outside.join(GENERATED_SKILL_MARKER_FILE).is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_preserves_user_owned_skill_and_agent_collisions() {
    let root = temp_dir("owned-collision");
    let codex_home = root.join("codex");
    let extension_dir = root.join("extension");
    fs::create_dir_all(extension_dir.join("skills/review")).unwrap();
    fs::create_dir_all(extension_dir.join("agents")).unwrap();
    fs::write(
        extension_dir.join("skills/review/SKILL.md"),
        "generated source",
    )
    .unwrap();
    fs::write(extension_dir.join("agents/reviewer.md"), "generated agent").unwrap();
    let skill_target = codex_home.join(".agents/skills/gemini-tools-review");
    fs::create_dir_all(&skill_target).unwrap();
    fs::write(skill_target.join("SKILL.md"), "user skill").unwrap();
    let agent_target = codex_home.join("agents/gemini-tools-reviewer.toml");
    fs::create_dir_all(agent_target.parent().unwrap()).unwrap();
    fs::write(&agent_target, "user agent").unwrap();
    let extensions = vec![GeminiExtension {
        directory: extension_dir,
        name: "tools".to_string(),
        value: serde_json::json!({}),
    }];

    assert!(write_gemini_skills(&codex_home, &extensions).is_err());
    assert_eq!(
        fs::read_to_string(skill_target.join("SKILL.md")).unwrap(),
        "user skill"
    );
    assert!(write_gemini_agents(&codex_home, &extensions).is_err());
    assert_eq!(fs::read_to_string(agent_target).unwrap(), "user agent");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_gemini_skill_source_does_not_remove_existing_generated_target() {
    let root = temp_dir("missing-skill");
    let codex_home = root.join("codex");
    let extension_dir = root.join("extension");
    fs::create_dir_all(extension_dir.join("skills/review")).unwrap();
    let target = codex_home.join(".agents/skills/gemini-tools-review");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(GENERATED_SKILL_MARKER_FILE), "tools").unwrap();
    fs::write(target.join("sentinel.txt"), "keep").unwrap();
    let extensions = vec![GeminiExtension {
        directory: extension_dir,
        name: "tools".to_string(),
        value: serde_json::json!({}),
    }];

    assert!(write_gemini_skills(&codex_home, &extensions).is_err());
    assert_eq!(
        fs::read_to_string(target.join("sentinel.txt")).unwrap(),
        "keep"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_preserves_user_prompt_and_hook_marker_text() {
    let root = temp_dir("user-marker-text");
    let codex_home = root.join("codex");
    let prompts_dir = codex_home.join("prompts");
    fs::create_dir_all(&prompts_dir).unwrap();
    let prompt_path = prompts_dir.join("notes.md");
    let prompt =
        format!("---\ndescription: User notes\n---\n\nThis documents {GENERATED_PROMPT_MARKER}.\n");
    fs::write(&prompt_path, &prompt).unwrap();
    let hooks_path = codex_home.join("hooks.json");
    fs::write(
        &hooks_path,
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "echo old-generated",
                            "statusMessage": "prodex-gemini-cli-compat: Gemini extension old"
                        },
                        {
                            "type": "command",
                            "command": "echo keep-me",
                            "statusMessage": "Gemini extension manual validation"
                        }
                    ]
                }]
            }
        })
        .to_string(),
    )
    .unwrap();

    write_gemini_prompts(&codex_home, &[], None).unwrap();
    write_gemini_hooks(&codex_home, &[], None).unwrap();

    assert_eq!(fs::read_to_string(prompt_path).unwrap(), prompt);
    let hooks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(hooks_path).unwrap()).unwrap();
    let commands = hooks["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0]["command"], "echo keep-me");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_deduplicates_normalized_skill_and_agent_names() {
    let root = temp_dir("normalized-collisions");
    let codex_home = root.join("codex");
    let extension_dir = root.join("extension");
    for (name, body) in [("foo bar", "skill one"), ("foo-bar", "skill two")] {
        let skill_dir = extension_dir.join("skills").join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), body).unwrap();
        fs::create_dir_all(extension_dir.join("agents")).unwrap();
        fs::write(
            extension_dir.join("agents").join(format!("{name}.md")),
            body,
        )
        .unwrap();
    }
    let extensions = vec![GeminiExtension {
        directory: extension_dir,
        name: "tools".to_string(),
        value: serde_json::json!({}),
    }];

    write_gemini_skills(&codex_home, &extensions).unwrap();
    write_gemini_agents(&codex_home, &extensions).unwrap();

    let skills_root = codex_home.join(".agents/skills");
    let agents_root = codex_home.join("agents");
    for (suffix, expected) in [("", "skill one"), ("-2", "skill two")] {
        let name = format!("gemini-tools-foo-bar{suffix}");
        assert!(
            fs::read_to_string(skills_root.join(&name).join("SKILL.md"))
                .unwrap()
                .contains(expected)
        );
        assert!(
            fs::read_to_string(agents_root.join(format!("{name}.toml")))
                .unwrap()
                .contains(expected)
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_serializes_agent_toml_and_skill_yaml() {
    let root = temp_dir("generated-serialization");
    let codex_home = root.join("codex");
    let extension_dir = root.join("extension");
    let skill_dir = extension_dir.join("skills/review");
    let agents_dir = extension_dir.join("agents");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "Review carefully.").unwrap();
    let agent_body = "# Reviewer\n\nC:\\temp\\queue\nline\t\"quoted\"\n";
    fs::write(agents_dir.join("review.md"), agent_body).unwrap();
    let extensions = vec![GeminiExtension {
        directory: extension_dir,
        name: "bad: [x".to_string(),
        value: serde_json::json!({}),
    }];

    write_gemini_skills(&codex_home, &extensions).unwrap();
    write_gemini_agents(&codex_home, &extensions).unwrap();

    let agent = fs::read_to_string(codex_home.join("agents/gemini-bad-x-review.toml")).unwrap();
    let parsed = toml::from_str::<toml::Value>(&agent).unwrap();
    assert_eq!(parsed["developer_instructions"].as_str(), Some(agent_body));
    let skill =
        fs::read_to_string(codex_home.join(".agents/skills/gemini-bad-x-review/SKILL.md")).unwrap();
    let description = skill
        .lines()
        .find_map(|line| line.strip_prefix("description: "))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<String>(description).unwrap(),
        "Gemini extension bad: [x skill review."
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn gemini_checkpoint_round_trips_staged_and_untracked_files() {
    use std::process::Command;

    let root = temp_dir("checkpoint-round-trip");
    let repo = root.join("repo");
    let codex_home = root.join("codex");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.name", "Prodex Test"]);
    git(&["config", "user.email", "developer@example.com"]);
    fs::write(repo.join("tracked.txt"), "base\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "--quiet", "-m", "test base"]);
    fs::write(repo.join("tracked.txt"), "staged change\n").unwrap();
    git(&["add", "tracked.txt"]);
    fs::write(repo.join("untracked.txt"), "untracked content\n").unwrap();
    write_gemini_admin_helpers(&codex_home).unwrap();

    let create = codex_home.join("bin/prodex-gemini-checkpoint-create");
    let output = Command::new(create)
        .arg("round-trip")
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let relative_diff = String::from_utf8(output.stdout).unwrap();
    let checkpoint = repo.join(relative_diff.trim());
    let patch = fs::read_to_string(&checkpoint).unwrap();
    assert!(patch.contains("+staged change"));
    assert!(patch.contains("+untracked content"));
    assert!(!patch.contains(".gemini/checkpoints"));

    git(&["reset", "--hard", "--quiet", "HEAD"]);
    fs::remove_file(repo.join("untracked.txt")).unwrap();
    let restore = codex_home.join("bin/prodex-gemini-checkpoint-restore");
    let output = Command::new(restore)
        .arg(&checkpoint)
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(repo.join("tracked.txt")).unwrap(),
        "staged change\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join("untracked.txt")).unwrap(),
        "untracked content\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_directory_copy_uses_one_global_entry_budget() {
    let root = temp_dir("copy-budget");
    let source = root.join("source");
    let target = root.join("target");
    for directory in 0..5 {
        let nested = source.join(format!("d{directory}"));
        fs::create_dir_all(&nested).unwrap();
        for file in 0..5 {
            fs::write(nested.join(format!("f{file}.txt")), "x").unwrap();
        }
    }
    fs::create_dir_all(&target).unwrap();

    crate::fs_utils::copy_dir_limited(&source, &target, 7).unwrap();
    let copied = crate::fs_utils::collect_files(&target, "txt", 100).unwrap();
    assert!(copied.len() <= 7, "copied {} files", copied.len());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_directory_scan_rejects_excessive_depth() {
    let root = temp_dir("scan-depth");
    let mut directory = root.join("source");
    fs::create_dir_all(&directory).unwrap();
    for index in 0..=GEMINI_EXTENSION_SCAN_LIMIT.min(40) {
        directory = directory.join(format!("d{index}"));
        fs::create_dir_all(&directory).unwrap();
    }
    fs::write(directory.join("agent.md"), "deep").unwrap();

    assert!(crate::fs_utils::collect_files(&root.join("source"), "md", 1_000).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_bridges_extension_mcp_commands_hooks_and_skills() {
    let root = temp_dir("full");
    let codex_home = root.join("codex");
    let extensions_root = root.join("extensions");
    let extension = extensions_root.join("workspace");
    fs::create_dir_all(extension.join("commands")).unwrap();
    fs::create_dir_all(extension.join("hooks")).unwrap();
    fs::create_dir_all(extension.join("agents")).unwrap();
    fs::create_dir_all(extension.join("skills").join("review")).unwrap();
    fs::write(
        extension.join("gemini-extension.json"),
        serde_json::json!({
            "name": "workspace-tools",
            "mcpServers": {
                "ctx": {
                    "command": "node",
                    "args": ["${extensionPath}/server.js"],
                    "env": {"TOKEN": "${WORKSPACE_TOKEN}"},
                    "envVars": ["WORKSPACE_TOKEN"],
                    "disabledTools": ["delete"]
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(extension.join(".env"), "WORKSPACE_TOKEN=secret\n").unwrap();
    fs::write(
        extension.join("commands").join("review.toml"),
        "description = \"Review code\"\nprompt = \"Review {{args.path}} with {{args}}\"\n",
    )
    .unwrap();
    fs::write(
        extension.join("hooks").join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "BeforeTool": [
                    {
                        "matcher": "run_shell_command",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "${extensionPath}/check.sh",
                                "statusMessage": "Checking shell"
                            }
                        ]
                    }
                ]
            }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        extension.join("skills").join("review").join("SKILL.md"),
        "---\nname: review\ndescription: review things\n---\n\nReview carefully.\n",
    )
    .unwrap();
    fs::write(
        extension.join("agents").join("reviewer.md"),
        "# Reviewer\n\nReview like Gemini CLI reviewer.",
    )
    .unwrap();

    let extensions =
        active_extension_manifests_from_roots(std::slice::from_ref(&extensions_root), None);
    write_gemini_mcp_config(&codex_home, &extensions, None).unwrap();
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Read",
                    "hooks": [{"type": "command", "command": "echo existing"}]
                }]
            }
        })
        .to_string(),
    )
    .unwrap();
    write_gemini_hooks(&codex_home, &extensions, None).unwrap();
    write_gemini_prompts(&codex_home, &extensions, None).unwrap();
    write_gemini_skills(&codex_home, &extensions).unwrap();
    write_gemini_agents(&codex_home, &extensions).unwrap();
    write_gemini_admin_helpers(&codex_home).unwrap();

    let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("[mcp_servers.gemini_workspace_tools_ctx]"));
    assert!(config.contains("WORKSPACE_TOKEN"));
    assert!(config.contains("TOKEN = \"secret\""));
    assert!(config.contains("disabled_tools = [\"delete\"]"));

    let hooks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(codex_home.join("hooks.json")).unwrap()).unwrap();
    let expected_status =
        "prodex-gemini-cli-compat: Gemini extension workspace-tools: Checking shell";
    let (hook_group, generated_hook) = hooks["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|group| {
            group["hooks"].as_array()?.iter().find_map(|hook| {
                (hook["statusMessage"].as_str() == Some(expected_status)).then_some((group, hook))
            })
        })
        .expect("workspace-tools hook should be generated");
    assert_eq!(
        hook_group["matcher"],
        serde_json::Value::String("Bash".to_string())
    );
    let hook_command = generated_hook["command"].as_str().unwrap();
    assert!(
        hook_command
            .replace('\\', "/")
            .ends_with("/workspace/check.sh"),
        "unexpected hook command: {hook_command}"
    );
    assert_eq!(generated_hook["statusMessage"], expected_status);

    let prompt =
        fs::read_to_string(codex_home.join("prompts").join("workspace-tools-review.md")).unwrap();
    assert!(prompt.contains("$PATH"));
    assert!(prompt.contains("$ARGUMENTS"));

    let skill = fs::read_to_string(
        codex_home
            .join(".agents")
            .join("skills")
            .join("gemini-workspace-tools-review")
            .join("SKILL.md"),
    )
    .unwrap();
    assert!(skill.contains("name: gemini-workspace-tools-review"));
    assert!(skill.contains("Review carefully."));
    let agent = fs::read_to_string(
        codex_home
            .join("agents")
            .join("gemini-workspace-tools-reviewer.toml"),
    )
    .unwrap();
    assert!(agent.contains("name = \"gemini-workspace-tools-reviewer\""));
    assert!(agent.contains("Review like Gemini CLI reviewer."));
    assert!(
        codex_home
            .join("bin")
            .join("prodex-gemini-refresh")
            .is_file()
    );
    let helper = codex_home.join("bin/prodex-gemini-refresh");
    fs::write(&helper, "#!/usr/bin/env sh\necho keep-me\n").unwrap();
    assert!(write_gemini_admin_helpers(&codex_home).is_err());
    assert!(fs::read_to_string(helper).unwrap().contains("keep-me"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_bridges_settings_mcp_over_extension_mcp_and_hooks() {
    let root = temp_dir("settings-mcp");
    let codex_home = root.join("codex");
    let workspace = root.join("repo");
    let extensions_root = root.join("extensions");
    let extension = extensions_root.join("workspace");
    fs::create_dir_all(&extension).unwrap();
    fs::create_dir_all(workspace.join(".gemini")).unwrap();
    fs::write(
        extension.join("gemini-extension.json"),
        serde_json::json!({
            "name": "workspace-tools",
            "mcpServers": {
                "ctx": {"command": "extension-server"},
                "extra": {"command": "extension-extra-server"}
            }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        workspace.join(".gemini").join("settings.json"),
        serde_json::json!({
            "mcp": {
                "allowed": ["ctx", "http"],
                "excluded": ["skip"]
            },
            "mcpServers": {
                "ctx": {"command": "settings-server", "args": ["--stdio"]},
                "http": {
                    "url": "https://legacy.example/sse",
                    "httpUrl": "https://http.example/mcp",
                    "timeout": 15000,
                    "includeTools": ["safe"],
                    "excludeTools": ["danger"],
                    "trust": true
                },
                "skip": {"command": "skip-server"}
            },
            "hooks": {
                "AfterTool": [{
                    "matcher": "shell",
                    "command": "echo done"
                }]
            }
        })
        .to_string(),
    )
    .unwrap();

    let extensions = active_extension_manifests_from_roots(
        std::slice::from_ref(&extensions_root),
        Some(&workspace),
    );
    write_gemini_mcp_config(&codex_home, &extensions, Some(&workspace)).unwrap();
    write_gemini_hooks(&codex_home, &extensions, Some(&workspace)).unwrap();

    let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("[mcp_servers.gemini_ctx]"));
    assert!(config.contains("settings-server"));
    assert!(config.contains("[mcp_servers.gemini_http]"));
    assert!(config.contains("url = \"https://http.example/mcp\""));
    assert!(config.contains("startup_timeout_sec = 15"));
    assert!(config.contains("tool_timeout_sec = 15"));
    assert!(config.contains("enabled_tools = [\"safe\"]"));
    assert!(config.contains("disabled_tools = [\"danger\"]"));
    assert!(config.contains("default_tools_approval_mode = \"approve\""));
    assert!(!config.contains("extension-server"));
    assert!(!config.contains("extension-extra-server"));
    assert!(!config.contains("skip-server"));

    let hooks: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(codex_home.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(
        hooks["hooks"]["PostToolUse"][0]["matcher"],
        serde_json::Value::String("Bash".to_string())
    );
    assert_eq!(
        hooks["hooks"]["PostToolUse"][0]["hooks"][0]["statusMessage"],
        serde_json::Value::String(format!(
            "prodex-gemini-cli-compat: Gemini extension project:{}: echo done",
            workspace.display()
        ))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_settings_paths_follow_gemini_cli_precedence() {
    let home = PathBuf::from("/tmp/prodex-gemini-home");
    let cwd = PathBuf::from("/tmp/prodex-gemini-workspace/repo/sub");
    let config_home = home.join(".gemini");
    let paths = gemini_settings_source_paths_for_config_home(
        Some(&config_home),
        Some(&cwd),
        Some(Path::new("/etc/gemini-cli/settings.json")),
        None,
    );
    let repo_settings = PathBuf::from("/tmp/prodex-gemini-workspace/repo")
        .join(".gemini")
        .join("settings.json");
    let sub_settings = cwd.join(".gemini").join("settings.json");

    assert_eq!(
        paths.first(),
        Some(&(
            "system-defaults".to_string(),
            PathBuf::from("/etc/gemini-cli/system-defaults.json")
        ))
    );
    assert_eq!(
        paths.get(1),
        Some(&(
            "global".to_string(),
            home.join(".gemini").join("settings.json")
        ))
    );
    assert!(
        paths.iter().position(|(_, path)| path == &repo_settings)
            < paths.iter().position(|(_, path)| path == &sub_settings)
    );
    assert_eq!(
        paths.get(paths.len().saturating_sub(2)),
        Some(&(
            format!("project-local:{}", cwd.display()),
            cwd.join(".gemini").join("settings.local.json")
        ))
    );
    assert_eq!(
        paths.last(),
        Some(&(
            "system".to_string(),
            PathBuf::from("/etc/gemini-cli/settings.json")
        ))
    );
    assert_eq!(
        paths.len(),
        paths
            .iter()
            .map(|(_, path)| path)
            .collect::<BTreeSet<_>>()
            .len(),
        "settings paths should be deduplicated"
    );
}

#[test]
fn gemini_cli_compat_settings_paths_honor_gemini_cli_home() {
    let paths = gemini_settings_source_paths_for_config_home(
        Some(Path::new("/tmp/gemini-cli-home/.gemini")),
        Some(Path::new("/tmp/workspace")),
        None,
        None,
    );

    assert!(paths.iter().any(|(_, path)| {
        path == &PathBuf::from("/tmp/gemini-cli-home")
            .join(".gemini")
            .join("settings.json")
    }));
    assert!(!paths.iter().any(|(_, path)| {
        path == &PathBuf::from("/tmp/plain-home")
            .join(".gemini")
            .join("settings.json")
    }));
}

#[test]
fn gemini_cli_compat_parses_commented_settings_json() {
    let value = parse_gemini_settings_json(
        r#"{
          // Gemini CLI settings permit comments.
          "mcpServers": {
            "ctx": {"command": "server"} /* inline block */
          }
        }"#,
    )
    .expect("commented settings should parse");

    assert_eq!(value["mcpServers"]["ctx"]["command"], "server");
}

#[test]
fn gemini_cli_compat_preserves_user_mcp_and_replaces_generated_entries() {
    let root = temp_dir("mcp-preserve");
    let codex_home = root.join("codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("config.toml"),
        "[mcp_servers.custom]\ncommand = \"custom\"\n\n[mcp_servers.old]\nprodex-gemini-cli-compat = \"old\"\ncommand = \"old\"\n",
    )
    .unwrap();
    write_gemini_mcp_config(&codex_home, &[], None).unwrap();
    let config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("[mcp_servers.custom]"));
    assert!(!config.contains("[mcp_servers.old]"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_preserves_oversized_existing_config() {
    let root = temp_dir("oversized-config");
    let codex_home = root.join("codex");
    fs::create_dir_all(&codex_home).unwrap();
    let config_path = codex_home.join("config.toml");
    let original = format!(
        "answer = 42\npadding = \"{}\"\n",
        "a".repeat(GEMINI_COMPAT_FILE_LIMIT)
    );
    fs::write(&config_path, &original).unwrap();

    let result = write_gemini_mcp_config(&codex_home, &[], None);

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn gemini_cli_compat_refuses_symlinked_config_write() {
    let root = temp_dir("config-symlink-write");
    let codex_home = root.join("codex");
    let outside = root.join("outside.toml");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(&outside, "do not touch").unwrap();
    std::os::unix::fs::symlink(&outside, codex_home.join("config.toml")).unwrap();

    let result = write_gemini_mcp_config(&codex_home, &[], None);

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "do not touch");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_preserves_oversized_existing_hooks_json() {
    let root = temp_dir("oversized-hooks");
    let codex_home = root.join("codex");
    fs::create_dir_all(&codex_home).unwrap();
    let hooks_path = codex_home.join("hooks.json");
    let original = serde_json::json!({
        "hooks": {},
        "padding": "a".repeat(GEMINI_COMPAT_FILE_LIMIT)
    })
    .to_string();
    fs::write(&hooks_path, &original).unwrap();

    let result = write_gemini_hooks(&codex_home, &[], None);

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&hooks_path).unwrap(), original);
    for invalid in ["[]", r#"{"hooks": []}"#] {
        fs::write(&hooks_path, invalid).unwrap();
        assert!(write_gemini_hooks(&codex_home, &[], None).is_err());
        assert_eq!(fs::read_to_string(&hooks_path).unwrap(), invalid);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_parses_gemini_placeholders() {
    assert_eq!(
        translate_gemini_prompt_placeholders("Use {{args.path}} and {{args}}"),
        "Use $PATH and $ARGUMENTS"
    );
}
