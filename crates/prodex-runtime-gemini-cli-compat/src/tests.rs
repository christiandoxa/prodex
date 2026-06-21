use super::*;
use crate::{gemini_settings_source_paths_for, parse_gemini_settings_json};
use std::sync::{Mutex, MutexGuard};

static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

struct TestEnvVarGuard {
    key: &'static str,
    old_value: Option<std::ffi::OsString>,
    _lock: Option<MutexGuard<'static, ()>>,
}

impl TestEnvVarGuard {
    fn lock() -> Self {
        Self {
            key: "",
            old_value: None,
            _lock: Some(TEST_ENV_LOCK.lock().unwrap()),
        }
    }

    fn set(key: &'static str, value: &str) -> Self {
        let old_value = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self {
            key,
            old_value,
            _lock: None,
        }
    }

    fn unset(key: &'static str) -> Self {
        let old_value = std::env::var_os(key);
        unsafe {
            std::env::remove_var(key);
        }
        Self {
            key,
            old_value,
            _lock: None,
        }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if self.key.is_empty() {
            return;
        }
        unsafe {
            if let Some(value) = &self.old_value {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-gemini-cli-compat-{name}-{stamp}"))
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
    assert_eq!(
        hooks["hooks"]["PreToolUse"][0]["matcher"],
        serde_json::Value::String("Bash".to_string())
    );
    assert!(
        hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("/workspace/check.sh")
    );
    assert_eq!(
        hooks["hooks"]["PreToolUse"][0]["hooks"][0]["statusMessage"],
        serde_json::Value::String("Gemini extension workspace-tools: Checking shell".to_string())
    );

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
            "Gemini extension project:{}: echo done",
            workspace.display()
        ))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_cli_compat_settings_paths_follow_gemini_cli_precedence() {
    let _env_lock = TestEnvVarGuard::lock();
    let _home_guard = TestEnvVarGuard::unset("GEMINI_CLI_HOME");
    let _system_guard = TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
    let _defaults_guard = TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
    let home = PathBuf::from("/tmp/prodex-gemini-home");
    let cwd = PathBuf::from("/tmp/prodex-gemini-workspace/repo/sub");
    let paths = gemini_settings_source_paths_for(Some(&home), Some(&cwd));
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
    let _env_lock = TestEnvVarGuard::lock();
    let _home_guard = TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gemini-cli-home");
    let _system_guard = TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
    let _defaults_guard = TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");

    let paths = gemini_settings_source_paths_for(
        Some(Path::new("/tmp/plain-home")),
        Some(Path::new("/tmp/workspace")),
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
fn gemini_cli_compat_parses_gemini_placeholders() {
    assert_eq!(
        translate_gemini_prompt_placeholders("Use {{args.path}} and {{args}}"),
        "Use $PATH and $ARGUMENTS"
    );
}
