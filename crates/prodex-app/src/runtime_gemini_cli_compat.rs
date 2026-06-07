use crate::{GeminiSettingsSource, gemini_cli_config_home_for, gemini_settings_sources};
#[cfg(test)]
use crate::{gemini_settings_source_paths_for, parse_gemini_settings_json};
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const GEMINI_EXTENSION_SCAN_LIMIT: usize = 64;
const GEMINI_COMPAT_FILE_LIMIT: usize = 512 * 1024;
const GENERATED_MARKER: &str = "prodex-gemini-cli-compat";
const GENERATED_PROMPT_MARKER: &str = "<!-- prodex-gemini-cli-compat generated prompt -->";
const GENERATED_SKILL_MARKER_FILE: &str = ".prodex-gemini-cli-compat";
const GEMINI_PROMPT_PREFIX: &str = "gemini";

#[derive(Debug, Clone)]
struct GeminiExtension {
    directory: PathBuf,
    name: String,
    value: serde_json::Value,
}

#[derive(Debug, Clone)]
struct GeneratedPrompt {
    slug: String,
    description: String,
    argument_hint: Option<String>,
    body: String,
}

pub(crate) fn prepare_gemini_cli_compat(codex_home: &Path) -> Result<()> {
    if gemini_env_bool("PRODEX_GEMINI_DISABLE_CLI_COMPAT") == Some(true) {
        return Ok(());
    }

    fs::create_dir_all(codex_home)
        .with_context(|| format!("failed to create {}", codex_home.display()))?;
    let cwd = env::current_dir().ok();
    let extensions = active_extension_manifests_from_roots(
        &gemini_extension_roots(cwd.as_deref()),
        cwd.as_deref(),
    );

    write_gemini_mcp_config(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_hooks(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_prompts(codex_home, &extensions, cwd.as_deref())?;
    write_gemini_skills(codex_home, &extensions)?;
    write_gemini_agents(codex_home, &extensions)?;
    write_gemini_admin_helpers(codex_home)?;
    Ok(())
}

pub(crate) fn handle_gemini_compat_refresh(args: crate::GeminiCompatRefreshArgs) -> Result<()> {
    prepare_gemini_cli_compat(&args.codex_home)?;
    println!(
        "Gemini CLI compatibility refreshed in {}",
        args.codex_home.display()
    );
    Ok(())
}

fn write_gemini_mcp_config(
    codex_home: &Path,
    extensions: &[GeminiExtension],
    cwd: Option<&Path>,
) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let mut table = read_toml_table(&config_path)?;
    remove_generated_mcp_servers(&mut table);
    let settings_sources = gemini_settings_sources(cwd);
    let (allowed_mcp_servers, excluded_mcp_servers) =
        gemini_mcp_settings_filters(&settings_sources);
    let settings_server_names = settings_sources
        .iter()
        .flat_map(|source| source.mcp_servers.keys())
        .map(|name| name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    for extension in extensions {
        let Some(servers) = extension
            .value
            .get("mcpServers")
            .or_else(|| extension.value.get("mcp_servers"))
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let extension_env = read_extension_env(&extension.directory);
        for (server_name, server_value) in servers {
            if !gemini_mcp_server_enabled_by_filters(
                server_name,
                allowed_mcp_servers.as_ref(),
                &excluded_mcp_servers,
            ) {
                continue;
            }
            if settings_server_names.contains(&server_name.to_ascii_lowercase()) {
                continue;
            }
            let Some(server_object) = server_value.as_object() else {
                continue;
            };
            let codex_server_name = format!(
                "{}_{}_{}",
                GEMINI_PROMPT_PREFIX,
                safe_key(&extension.name),
                safe_key(server_name)
            );
            configure_gemini_mcp_server(
                &mut table,
                &codex_server_name,
                &extension.name,
                &extension.directory,
                server_object,
                &extension_env,
                cwd,
            );
        }
    }

    for settings in settings_sources {
        for (server_name, server_value) in settings.mcp_servers {
            if !gemini_mcp_server_enabled_by_filters(
                &server_name,
                allowed_mcp_servers.as_ref(),
                &excluded_mcp_servers,
            ) {
                continue;
            }
            let Some(server_object) = server_value.as_object() else {
                continue;
            };
            let codex_server_name = format!("{}_{}", GEMINI_PROMPT_PREFIX, safe_key(&server_name));
            configure_gemini_mcp_server(
                &mut table,
                &codex_server_name,
                &settings.name,
                &settings.directory,
                server_object,
                &BTreeMap::new(),
                cwd,
            );
        }
    }

    write_toml_table(
        &config_path,
        table,
        "Gemini CLI compatibility config overlay",
    )
}

fn configure_gemini_mcp_server(
    root_table: &mut toml::Table,
    server_name: &str,
    source_name: &str,
    source_directory: &Path,
    server: &serde_json::Map<String, serde_json::Value>,
    extension_env: &BTreeMap<String, String>,
    cwd: Option<&Path>,
) {
    let Some(mcp_servers) = ensure_child_table(root_table, "mcp_servers") else {
        return;
    };
    if mcp_servers
        .get(server_name)
        .and_then(toml::Value::as_table)
        .is_some_and(|server| server.get(GENERATED_MARKER).is_none())
    {
        return;
    }

    let mut codex_server = toml::Table::new();
    codex_server.insert(
        GENERATED_MARKER.to_string(),
        toml::Value::String(source_name.to_string()),
    );
    let vars = GeminiCompatVars::new(source_directory, cwd).with_env(extension_env);

    if let Some(url) = json_string(server, &["httpUrl", "http_url", "url"]) {
        codex_server.insert("url".to_string(), toml::Value::String(vars.expand(&url)));
        insert_optional_string(
            &mut codex_server,
            "bearer_token_env_var",
            json_string(
                server,
                &[
                    "bearerTokenEnvVar",
                    "bearer_token_env_var",
                    "oauthTokenEnvVar",
                ],
            )
            .as_deref(),
            &vars,
        );
        insert_string_map(
            &mut codex_server,
            "http_headers",
            server
                .get("http_headers")
                .or_else(|| server.get("httpHeaders"))
                .or_else(|| server.get("headers")),
            &vars,
        );
        insert_string_map(
            &mut codex_server,
            "env_http_headers",
            server
                .get("env_http_headers")
                .or_else(|| server.get("envHttpHeaders")),
            &vars,
        );
    } else if let Some(command) = json_string(server, &["command", "cmd"]) {
        codex_server.insert(
            "command".to_string(),
            toml::Value::String(vars.expand(&command)),
        );
        if let Some(args) = json_string_array(server.get("args")) {
            codex_server.insert(
                "args".to_string(),
                toml::Value::Array(
                    args.into_iter()
                        .map(|arg| toml::Value::String(vars.expand(&arg)))
                        .collect(),
                ),
            );
        }
        insert_optional_string(
            &mut codex_server,
            "cwd",
            json_string(server, &["cwd", "workingDirectory", "working_directory"]).as_deref(),
            &vars,
        );
    } else {
        return;
    }

    if let Some(enabled) = json_bool(server, &["enabled"]) {
        codex_server.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    } else if let Some(disabled) = json_bool(server, &["disabled"]) {
        codex_server.insert("enabled".to_string(), toml::Value::Boolean(!disabled));
    }
    if let Some(required) = json_bool(server, &["required"]) {
        codex_server.insert("required".to_string(), toml::Value::Boolean(required));
    }
    insert_optional_number(
        &mut codex_server,
        "startup_timeout_sec",
        gemini_mcp_timeout_seconds(
            server,
            &["startupTimeoutSec", "startup_timeout_sec", "timeoutSec"],
        ),
    );
    insert_optional_number(
        &mut codex_server,
        "tool_timeout_sec",
        gemini_mcp_timeout_seconds(server, &["toolTimeoutSec", "tool_timeout_sec"]),
    );
    insert_optional_string_array(
        &mut codex_server,
        "enabled_tools",
        server
            .get("enabled_tools")
            .or_else(|| server.get("enabledTools"))
            .or_else(|| server.get("includeTools")),
        &vars,
    );
    insert_optional_string_array(
        &mut codex_server,
        "disabled_tools",
        server
            .get("disabled_tools")
            .or_else(|| server.get("disabledTools"))
            .or_else(|| server.get("excludeTools")),
        &vars,
    );
    if let Some(mode) = json_string(
        server,
        &["defaultToolsApprovalMode", "default_tools_approval_mode"],
    ) {
        codex_server.insert(
            "default_tools_approval_mode".to_string(),
            toml::Value::String(vars.expand(&mode)),
        );
    } else if json_bool(server, &["trust"]).unwrap_or(false) {
        codex_server.insert(
            "default_tools_approval_mode".to_string(),
            toml::Value::String("approve".to_string()),
        );
    }

    let mut env_table = toml::Table::new();
    insert_env_map(&mut env_table, server.get("env"), &vars);
    for name in json_string_array(
        server
            .get("envVars")
            .or_else(|| server.get("env_vars"))
            .or_else(|| server.get("env")),
    )
    .unwrap_or_default()
    {
        if let Some(value) = extension_env
            .get(&name)
            .cloned()
            .or_else(|| env::var(&name).ok())
        {
            env_table.insert(name, toml::Value::String(vars.expand(&value)));
        }
    }
    for (key, value) in extension_env {
        if placeholder_mentioned(server, key) {
            env_table
                .entry(key.clone())
                .or_insert_with(|| toml::Value::String(vars.expand(value)));
        }
    }
    if !env_table.is_empty() {
        codex_server.insert("env".to_string(), toml::Value::Table(env_table));
    }

    mcp_servers.insert(server_name.to_string(), toml::Value::Table(codex_server));
}

fn write_gemini_hooks(
    codex_home: &Path,
    extensions: &[GeminiExtension],
    cwd: Option<&Path>,
) -> Result<()> {
    let mut generated = BTreeMap::<String, Vec<serde_json::Value>>::new();
    for extension in extensions {
        let hook_sources = extension_hook_sources(extension);
        for source in hook_sources {
            collect_codex_hooks_from_value(extension, &source, cwd, &mut generated);
        }
    }
    for settings in gemini_settings_sources(cwd) {
        let pseudo_extension = GeminiExtension {
            directory: settings.directory.clone(),
            name: settings.name.clone(),
            value: settings.value.clone(),
        };
        for source in settings_hook_sources(&settings) {
            collect_codex_hooks_from_value(&pseudo_extension, &source, cwd, &mut generated);
        }
    }

    let hooks_path = codex_home.join("hooks.json");
    let mut hooks_root = read_hooks_json(&hooks_path)?;
    remove_generated_hooks(&mut hooks_root);
    let has_generated_hooks = !generated.is_empty();
    if has_generated_hooks {
        let root_object = hooks_root
            .as_object_mut()
            .expect("hooks root should be an object after read");
        let hooks_value = root_object
            .entry("hooks".to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        let Some(hooks_object) = hooks_value.as_object_mut() else {
            return Ok(());
        };
        for (event, groups) in generated {
            let event_value = hooks_object
                .entry(event)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
            if let Some(event_groups) = event_value.as_array_mut() {
                event_groups.extend(groups);
            }
        }
    }

    if hooks_root
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|hooks| {
            hooks
                .values()
                .any(|value| value.as_array().is_some_and(|items| !items.is_empty()))
        })
    {
        let rendered = serde_json::to_string_pretty(&hooks_root)
            .context("failed to serialize generated Gemini hooks")?;
        fs::write(&hooks_path, rendered)
            .with_context(|| format!("failed to write {}", hooks_path.display()))?;
    } else if hooks_path.exists() {
        let rendered = serde_json::to_string_pretty(&hooks_root)
            .context("failed to serialize hooks after Gemini cleanup")?;
        fs::write(&hooks_path, rendered)
            .with_context(|| format!("failed to write {}", hooks_path.display()))?;
    }

    if has_generated_hooks {
        enable_codex_hooks_feature(codex_home)?;
    }
    Ok(())
}

fn extension_hook_sources(extension: &GeminiExtension) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    if let Some(hooks) = extension.value.get("hooks") {
        values.push(hooks.clone());
    }
    for relative in [
        Path::new("hooks").join("hooks.json"),
        PathBuf::from("hooks.json"),
    ] {
        let path = extension.directory.join(relative);
        let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            values.push(value);
        }
    }
    values
}

fn collect_codex_hooks_from_value(
    extension: &GeminiExtension,
    value: &serde_json::Value,
    cwd: Option<&Path>,
    generated: &mut BTreeMap<String, Vec<serde_json::Value>>,
) {
    let vars = GeminiCompatVars::new(&extension.directory, cwd);
    let hook_map = value.get("hooks").unwrap_or(value);
    let Some(events) = hook_map.as_object() else {
        return;
    };
    for (event_name, groups_value) in events {
        let Some(codex_event) = codex_hook_event(event_name) else {
            continue;
        };
        let groups = if let Some(array) = groups_value.as_array() {
            array.clone()
        } else {
            vec![groups_value.clone()]
        };
        for group in groups {
            let Some(codex_group) = codex_hook_group(extension, &group, &vars) else {
                continue;
            };
            generated
                .entry(codex_event.to_string())
                .or_default()
                .push(codex_group);
        }
    }
}

fn codex_hook_group(
    extension: &GeminiExtension,
    group: &serde_json::Value,
    vars: &GeminiCompatVars,
) -> Option<serde_json::Value> {
    let group_object = group.as_object()?;
    let mut codex_hooks = Vec::new();
    if let Some(hooks) = group_object
        .get("hooks")
        .and_then(serde_json::Value::as_array)
    {
        for hook in hooks {
            if let Some(command_hook) = codex_command_hook(extension, hook, vars) {
                codex_hooks.push(command_hook);
            }
        }
    } else if let Some(command_hook) = codex_command_hook(extension, group, vars) {
        codex_hooks.push(command_hook);
    }
    if codex_hooks.is_empty() {
        return None;
    }

    let mut output = serde_json::Map::new();
    if let Some(matcher) = group_object
        .get("matcher")
        .or_else(|| group_object.get("tool"))
        .or_else(|| group_object.get("toolName"))
        .and_then(serde_json::Value::as_str)
    {
        output.insert(
            "matcher".to_string(),
            serde_json::Value::String(codex_hook_matcher(matcher)),
        );
    }
    output.insert("hooks".to_string(), serde_json::Value::Array(codex_hooks));
    Some(serde_json::Value::Object(output))
}

fn codex_command_hook(
    extension: &GeminiExtension,
    hook: &serde_json::Value,
    vars: &GeminiCompatVars,
) -> Option<serde_json::Value> {
    let hook_object = hook.as_object()?;
    let hook_type = hook_object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("command");
    if !hook_type.eq_ignore_ascii_case("command") {
        return None;
    }
    if hook_object
        .get("async")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let command = hook_object
        .get("command")
        .or_else(|| hook_object.get("cmd"))
        .and_then(serde_json::Value::as_str)?;
    let mut output = serde_json::Map::new();
    output.insert("type".to_string(), json!("command"));
    output.insert("command".to_string(), json!(vars.expand(command)));
    if let Some(status) = hook_object
        .get("statusMessage")
        .or_else(|| hook_object.get("status_message"))
        .or_else(|| hook_object.get("description"))
        .and_then(serde_json::Value::as_str)
    {
        output.insert(
            "statusMessage".to_string(),
            json!(format!(
                "Gemini extension {}: {}",
                extension.name,
                vars.expand(status)
            )),
        );
    } else {
        output.insert(
            "statusMessage".to_string(),
            json!(format!(
                "Gemini extension {}: {}",
                extension.name,
                vars.expand(command)
            )),
        );
    }
    if let Some(timeout) = hook_object
        .get("timeout")
        .or_else(|| hook_object.get("timeoutSec"))
        .and_then(serde_json::Value::as_u64)
    {
        output.insert("timeout".to_string(), json!(timeout));
    }
    if let Some(command_windows) = hook_object
        .get("commandWindows")
        .or_else(|| hook_object.get("command_windows"))
        .and_then(serde_json::Value::as_str)
    {
        output.insert(
            "commandWindows".to_string(),
            json!(vars.expand(command_windows)),
        );
    }
    Some(serde_json::Value::Object(output))
}

fn codex_hook_event(event: &str) -> Option<&'static str> {
    let normalized = event
        .trim()
        .replace(['-', '_', ' '], "")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "pretooluse" | "beforetool" | "beforetooluse" | "toolstart" => Some("PreToolUse"),
        "permissionrequest" | "toolconfirmation" | "beforetoolconfirmation" => {
            Some("PermissionRequest")
        }
        "posttooluse" | "aftertool" | "aftertooluse" | "toolfinish" => Some("PostToolUse"),
        "precompact" | "beforecompact" => Some("PreCompact"),
        "postcompact" | "aftercompact" => Some("PostCompact"),
        "userpromptsubmit" | "beforeagent" | "promptsubmit" | "userprompt" => {
            Some("UserPromptSubmit")
        }
        "subagentstart" => Some("SubagentStart"),
        "subagentstop" => Some("SubagentStop"),
        "stop" | "afteragent" | "sessionend" => Some("Stop"),
        "sessionstart" => Some("SessionStart"),
        _ => None,
    }
}

fn codex_hook_matcher(matcher: &str) -> String {
    let normalized = matcher.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "run_shell_command" | "shell" | "bash" | "exec_command" => "Bash".to_string(),
        "write_file" | "edit" | "replace" | "apply_patch" => "apply_patch|Edit|Write".to_string(),
        "*" | "" => "*".to_string(),
        _ => matcher.to_string(),
    }
}

fn write_gemini_prompts(
    codex_home: &Path,
    extensions: &[GeminiExtension],
    cwd: Option<&Path>,
) -> Result<()> {
    let prompts_dir = codex_home.join("prompts");
    fs::create_dir_all(&prompts_dir)
        .with_context(|| format!("failed to create {}", prompts_dir.display()))?;
    remove_generated_prompt_files(&prompts_dir)?;

    let mut prompts = builtin_gemini_prompts();
    prompts.extend(collect_user_and_project_commands(cwd));
    for extension in extensions {
        prompts.extend(collect_extension_commands(extension, cwd));
        prompts.extend(collect_extension_agent_prompts(extension));
        prompts.extend(collect_extension_skill_prompts(extension));
    }

    let mut seen = existing_prompt_slugs(&prompts_dir);
    for mut prompt in prompts {
        prompt.slug = unique_slug(&prompt.slug, &mut seen);
        write_prompt_file(&prompts_dir, &prompt)?;
    }
    Ok(())
}

fn builtin_gemini_prompts() -> Vec<GeneratedPrompt> {
    vec![
        GeneratedPrompt {
            slug: "gemini-refresh".to_string(),
            description: "Refresh Gemini CLI compatibility projection in Codex home.".to_string(),
            argument_hint: Some("[--dry-run]".to_string()),
            body: concat!(
                "Refresh the Gemini CLI compatibility projection for this Codex profile.\n\n",
                "Run `prodex-gemini-refresh $ARGUMENTS` from the shell. This regenerates ",
                "Gemini settings/extension MCP servers, hooks, custom prompts, skills, custom agents, ",
                "and helper scripts in the active `CODEX_HOME`. Restart Codex or start a new chat ",
                "when you need newly generated prompt/skill/agent menus to be reloaded."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-extensions".to_string(),
            description: "Inspect Gemini extension compatibility state.".to_string(),
            argument_hint: Some("[FILTER=\"...\"]".to_string()),
            body: concat!(
                "Inspect Gemini extension compatibility for `$FILTER`.\n\n",
                "Check `~/.gemini/extensions`, project `.gemini/extensions`, ",
                "`extension-enablement.json`, generated Codex `config.toml` MCP entries named ",
                "`gemini_*`, generated `hooks.json` entries with `Gemini extension ...` status, ",
                "`prompts/*.md`, `.agents/skills`, and `agents/*.toml`. Do not edit unrelated files."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-mcp-reload".to_string(),
            description: "Refresh Gemini MCP settings and extension servers.".to_string(),
            argument_hint: Some("[SERVER=\"...\"]".to_string()),
            body: concat!(
                "Refresh Gemini MCP compatibility for `$SERVER`.\n\n",
                "Run `prodex-gemini-refresh` and then inspect generated `[mcp_servers.gemini_*]` ",
                "entries in `config.toml`. If Codex is already running, restart the session before ",
                "expecting new MCP tools to appear."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-show".to_string(),
            description: "Show effective Gemini memory files for this workspace.".to_string(),
            argument_hint: Some("[SCOPE=all|project|global]".to_string()),
            body: concat!(
                "Show effective Gemini memory for scope `$SCOPE`.\n\n",
                "Read, summarize, and report the relevant files without modifying them: ",
                "`~/.gemini/GEMINI.md`, `~/.gemini/memory/INBOX.md`, ancestor `GEMINI.md` files, ",
                "`.gemini/memory/MEMORY.md`, `.gemini/memory/INBOX.md`, and active extension ",
                "`contextFileName` files."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-refresh".to_string(),
            description: "Refresh and reconcile Gemini memory for this workspace.".to_string(),
            argument_hint: Some("[SCOPE=project|global|all]".to_string()),
            body: concat!(
                "Refresh Gemini memory for scope `$SCOPE`.\n\n",
                "Review the memory inbox, remove stale/transient notes, keep stable non-secret guidance, ",
                "and write concise updates to the appropriate Gemini memory file. Preserve unrelated content."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-inbox".to_string(),
            description: "Review and reconcile Gemini memory inbox notes.".to_string(),
            argument_hint: Some("[SCOPE=project|global]".to_string()),
            body: concat!(
                "Review Gemini memory inbox notes for this workspace.\n\n",
                "- Project inbox: `.gemini/memory/INBOX.md`\n",
                "- Project private memory: `.gemini/memory/MEMORY.md`\n",
                "- Global inbox: `~/.gemini/memory/INBOX.md`\n",
                "- Global Gemini memory: `~/.gemini/GEMINI.md`\n\n",
                "Move stable, non-secret, recurring guidance into the appropriate Gemini memory file. ",
                "Keep transient notes out of durable memory. Scope hint: $SCOPE."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-remember".to_string(),
            description: "Add stable guidance to the Gemini memory inbox.".to_string(),
            argument_hint: Some("SCOPE=project|global NOTE=\"...\"".to_string()),
            body: concat!(
                "Add this stable memory note to Gemini memory without storing secrets.\n\n",
                "Scope: $SCOPE\n",
                "Note: $NOTE\n\n",
                "Use `.gemini/memory/INBOX.md` for project-scoped notes and ",
                "`~/.gemini/memory/INBOX.md` for global notes. Create parent directories if needed. ",
                "Keep entries concise and actionable."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-create".to_string(),
            description: "Create a Gemini-style workspace checkpoint.".to_string(),
            argument_hint: Some("[NAME=label]".to_string()),
            body: concat!(
                "Create a Gemini-style workspace checkpoint.\n\n",
                "Run `prodex-gemini-checkpoint-create $NAME`. It records git HEAD, status, ",
                "and a binary diff under `.gemini/checkpoints/`. Use this before risky mutating work."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-export".to_string(),
            description: "Prepare a Gemini-compatible request checkpoint export.".to_string(),
            argument_hint: Some("FILE=checkpoint.json".to_string()),
            body: concat!(
                "Prepare this session to export Gemini bridge checkpoints.\n\n",
                "Use `PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE=$FILE` or request metadata ",
                "`gemini_checkpoint_export_file` on the next Gemini-backed request. ",
                "The bridge writes the translated Gemini `generateContent` request with model, ",
                "contents, tools, generation config, and system instruction for later import."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-restore".to_string(),
            description: "Restore a Gemini-style workspace checkpoint.".to_string(),
            argument_hint: Some("FILE=.gemini/checkpoints/name.diff".to_string()),
            body: concat!(
                "Restore a Gemini-style workspace checkpoint from `$FILE`.\n\n",
                "First inspect current `git status --short` and the checkpoint metadata. ",
                "If the user confirms restoration, run `prodex-gemini-checkpoint-restore $FILE`. ",
                "Do not discard unrelated user changes without explicit confirmation."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-rewind".to_string(),
            description: "Resume context from a Gemini request checkpoint file.".to_string(),
            argument_hint: Some("FILE=checkpoint.json".to_string()),
            body: concat!(
                "Plan a Gemini checkpoint rewind from `$FILE`.\n\n",
                "A future Gemini-backed run can import it with `PRODEX_GEMINI_CHECKPOINT_FILE=$FILE` ",
                "or request metadata `gemini_checkpoint_file`. Verify the file exists, summarize ",
                "what context it contains if readable, and avoid editing unrelated project files."
            )
            .to_string(),
        },
    ]
}

fn collect_user_and_project_commands(cwd: Option<&Path>) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    if let Some(home) = dirs::home_dir() {
        prompts.extend(collect_commands_from_dir(
            &home.join(".gemini").join("commands"),
            "",
            "user Gemini command",
            None,
        ));
    }
    if let Some(cwd) = cwd {
        prompts.extend(collect_commands_from_dir(
            &cwd.join(".gemini").join("commands"),
            "",
            "project Gemini command",
            None,
        ));
    }
    prompts
}

fn collect_extension_commands(
    extension: &GeminiExtension,
    cwd: Option<&Path>,
) -> Vec<GeneratedPrompt> {
    collect_commands_from_dir(
        &extension.directory.join("commands"),
        &safe_slug(&extension.name),
        "Gemini extension command",
        Some(&GeminiCompatVars::new(&extension.directory, cwd)),
    )
}

fn collect_commands_from_dir(
    directory: &Path,
    prefix: &str,
    source_label: &str,
    vars: Option<&GeminiCompatVars>,
) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    let Ok(paths) = collect_files(directory, "toml", GEMINI_EXTENSION_SCAN_LIMIT) else {
        return prompts;
    };
    for path in paths {
        let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        let relative = path.strip_prefix(directory).unwrap_or(path.as_path());
        let command_name = toml_string(&value, &["name"])
            .unwrap_or_else(|| relative.with_extension("").display().to_string());
        let slug = if prefix.is_empty() {
            safe_slug(&command_name)
        } else {
            format!("{}-{}", prefix, safe_slug(&command_name))
        };
        let description = toml_string(&value, &["description"])
            .unwrap_or_else(|| format!("{source_label}: {}", relative.display()));
        let argument_hint = toml_string(&value, &["argument-hint", "argumentHint"]);
        let raw_body = toml_string(&value, &["prompt", "body", "instruction", "instructions"])
            .or_else(|| {
                toml_string(&value, &["command"]).map(|command| {
                    format!(
                        "Run or adapt this Gemini command for the user's request:\n\n```sh\n{command}\n```"
                    )
                })
            })
            .unwrap_or_default();
        if raw_body.trim().is_empty() {
            continue;
        }
        let body = vars.map(|vars| vars.expand(&raw_body)).unwrap_or(raw_body);
        prompts.push(GeneratedPrompt {
            slug,
            description: description.clone(),
            argument_hint: argument_hint.clone(),
            body: translate_gemini_prompt_placeholders(&body),
        });
        for alias in toml_string_array(&value, &["aliases", "alias"]) {
            let alias_slug = if prefix.is_empty() {
                safe_slug(&alias)
            } else {
                format!("{}-{}", prefix, safe_slug(&alias))
            };
            prompts.push(GeneratedPrompt {
                slug: alias_slug,
                description: format!("{description} (alias)"),
                argument_hint: argument_hint.clone(),
                body: translate_gemini_prompt_placeholders(&body),
            });
        }
    }
    prompts
}

fn collect_extension_agent_prompts(extension: &GeminiExtension) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    let Ok(paths) = collect_files(
        &extension.directory.join("agents"),
        "md",
        GEMINI_EXTENSION_SCAN_LIMIT,
    ) else {
        return prompts;
    };
    for path in paths {
        let Some(body) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        let slug = format!(
            "gemini-{}-agent-{}",
            safe_slug(&extension.name),
            safe_slug(
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("agent")
            )
        );
        prompts.push(GeneratedPrompt {
            slug,
            description: format!("Use Gemini extension {} agent guidance.", extension.name),
            argument_hint: Some("[TASK=\"...\"]".to_string()),
            body: format!(
                "Use this Gemini extension agent guidance for the task `$TASK`.\n\n{body}"
            ),
        });
    }
    prompts
}

fn collect_extension_skill_prompts(extension: &GeminiExtension) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    for skill_dir in extension_skill_dirs(extension) {
        let skill_name = skill_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("skill");
        let Some(body) = read_text_limited(&skill_dir.join("SKILL.md"), GEMINI_COMPAT_FILE_LIMIT)
        else {
            continue;
        };
        prompts.push(GeneratedPrompt {
            slug: format!(
                "gemini-{}-skill-{}",
                safe_slug(&extension.name),
                safe_slug(skill_name)
            ),
            description: format!(
                "Invoke Gemini extension {} skill {}.",
                extension.name, skill_name
            ),
            argument_hint: Some("[TASK=\"...\"]".to_string()),
            body: format!("Use this Gemini extension skill for `$TASK`.\n\n{body}"),
        });
    }
    prompts
}

fn write_prompt_file(prompts_dir: &Path, prompt: &GeneratedPrompt) -> Result<()> {
    let path = prompts_dir.join(format!("{}.md", prompt.slug));
    let mut contents = String::new();
    contents.push_str(GENERATED_PROMPT_MARKER);
    contents.push('\n');
    contents.push_str("---\n");
    contents.push_str(&format!(
        "description: {}\n",
        yaml_string_literal(&prompt.description)
    ));
    if let Some(argument_hint) = &prompt.argument_hint {
        contents.push_str(&format!(
            "argument-hint: {}\n",
            yaml_string_literal(argument_hint)
        ));
    }
    contents.push_str("---\n\n");
    contents.push_str(&prompt.body);
    contents.push('\n');
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn write_gemini_skills(codex_home: &Path, extensions: &[GeminiExtension]) -> Result<()> {
    let skills_root = codex_home.join(".agents").join("skills");
    fs::create_dir_all(&skills_root)
        .with_context(|| format!("failed to create {}", skills_root.display()))?;
    remove_generated_skill_dirs(&skills_root)?;

    for extension in extensions {
        for skill_dir in extension_skill_dirs(extension) {
            let source_name = skill_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill");
            let target_name = format!(
                "gemini-{}-{}",
                safe_slug(&extension.name),
                safe_slug(source_name)
            );
            let target_dir = skills_root.join(target_name);
            copy_skill_dir(extension, &skill_dir, &target_dir, source_name)?;
        }
    }
    Ok(())
}

fn write_gemini_agents(codex_home: &Path, extensions: &[GeminiExtension]) -> Result<()> {
    let agents_root = codex_home.join("agents");
    fs::create_dir_all(&agents_root)
        .with_context(|| format!("failed to create {}", agents_root.display()))?;
    remove_generated_agent_files(&agents_root)?;

    for extension in extensions {
        let Ok(paths) = collect_files(
            &extension.directory.join("agents"),
            "md",
            GEMINI_EXTENSION_SCAN_LIMIT,
        ) else {
            continue;
        };
        for path in paths {
            let Some(body) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
                continue;
            };
            let source_name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("agent");
            let agent_name = format!(
                "gemini-{}-{}",
                safe_slug(&extension.name),
                safe_slug(source_name)
            );
            let description = first_nonempty_line(&body).unwrap_or_else(|| {
                format!("Gemini extension {} agent {}.", extension.name, source_name)
            });
            let rendered = format!(
                "# {GENERATED_MARKER}\nname = {name}\ndescription = {description}\ndeveloper_instructions = {instructions}\n",
                name = toml_string_literal(&agent_name),
                description = toml_string_literal(&description),
                instructions = toml_multiline_string_literal(&strip_front_matter(&body)),
            );
            fs::write(agents_root.join(format!("{agent_name}.toml")), rendered)
                .with_context(|| format!("failed to write generated agent {agent_name}"))?;
        }
    }
    Ok(())
}

fn write_gemini_admin_helpers(codex_home: &Path) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("prodex"));
    write_executable_script(
        &bin_dir.join("prodex-gemini-refresh"),
        &format!(
            "#!/usr/bin/env sh\nexec {} __gemini-compat-refresh --codex-home {} \"$@\"\n",
            shell_quote(&current_exe.display().to_string()),
            shell_quote(&codex_home.display().to_string())
        ),
    )?;
    write_executable_script(
        &bin_dir.join("prodex-gemini-checkpoint-create"),
        r#"#!/usr/bin/env sh
set -eu
name="${1:-checkpoint}"
safe="$(printf '%s' "$name" | tr -c 'A-Za-z0-9._-' '-')"
dir=".gemini/checkpoints"
mkdir -p "$dir"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
base="$dir/${stamp}-${safe}"
git rev-parse --show-toplevel >/dev/null 2>&1 || {
  echo "not inside a git worktree" >&2
  exit 2
}
{
  printf '{\n'
  printf '  "format": "prodex-gemini-worktree-checkpoint",\n'
  printf '  "created_at": "%s",\n' "$stamp"
  printf '  "head": "%s",\n' "$(git rev-parse HEAD 2>/dev/null || true)"
  printf '  "diff": "%s.diff",\n' "$(basename "$base")"
  printf '  "status": "%s.status"\n' "$(basename "$base")"
  printf '}\n'
} > "$base.json"
git status --porcelain=v1 > "$base.status"
git diff --binary > "$base.diff"
printf '%s\n' "$base.diff"
"#,
    )?;
    write_executable_script(
        &bin_dir.join("prodex-gemini-checkpoint-restore"),
        r#"#!/usr/bin/env sh
set -eu
file="${1:-}"
if [ -z "$file" ]; then
  echo "usage: prodex-gemini-checkpoint-restore .gemini/checkpoints/<checkpoint>.diff" >&2
  exit 2
fi
git rev-parse --show-toplevel >/dev/null 2>&1 || {
  echo "not inside a git worktree" >&2
  exit 2
}
if [ -n "$(git status --porcelain=v1)" ]; then
  echo "worktree is dirty; inspect status and confirm before restoring" >&2
  exit 3
fi
git apply --index "$file"
git reset
"#,
    )?;
    Ok(())
}

fn copy_skill_dir(
    extension: &GeminiExtension,
    source_dir: &Path,
    target_dir: &Path,
    source_name: &str,
) -> Result<()> {
    if target_dir.exists() {
        fs::remove_dir_all(target_dir)
            .with_context(|| format!("failed to remove {}", target_dir.display()))?;
    }
    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    fs::write(
        target_dir.join(GENERATED_SKILL_MARKER_FILE),
        &extension.name,
    )
    .with_context(|| format!("failed to mark {}", target_dir.display()))?;
    copy_dir_limited(source_dir, target_dir, GEMINI_EXTENSION_SCAN_LIMIT)?;
    let Some(source_skill) =
        read_text_limited(&source_dir.join("SKILL.md"), GEMINI_COMPAT_FILE_LIMIT)
    else {
        bail!(
            "Gemini extension skill missing {}",
            source_dir.join("SKILL.md").display()
        );
    };
    let skill_name = format!(
        "gemini-{}-{}",
        safe_slug(&extension.name),
        safe_slug(source_name)
    );
    let rewritten = format!(
        "---\nname: {skill_name}\ndescription: Gemini extension {} skill {}.\n---\n\n{}\n\n{}",
        extension.name,
        source_name,
        GENERATED_PROMPT_MARKER,
        strip_front_matter(&source_skill)
    );
    fs::write(target_dir.join("SKILL.md"), rewritten)
        .with_context(|| format!("failed to write {}", target_dir.join("SKILL.md").display()))?;
    Ok(())
}

fn extension_skill_dirs(extension: &GeminiExtension) -> Vec<PathBuf> {
    let skills_dir = extension.directory.join("skills");
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return Vec::new();
    };
    let mut dirs = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("SKILL.md").is_file())
        .take(GEMINI_EXTENSION_SCAN_LIMIT)
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn gemini_extension_roots(cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(configured) = env::var_os("PRODEX_GEMINI_EXTENSION_DIRS") {
        roots.extend(env::split_paths(&configured));
    }
    if let Some(gemini_home) = gemini_cli_config_home_for(dirs::home_dir().as_deref()) {
        roots.push(gemini_home.join("extensions"));
    }
    if let Some(cwd) = cwd {
        roots.push(cwd.join(".gemini").join("extensions"));
    }
    dedupe_paths(roots)
}

fn active_extension_manifests_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<GeminiExtension> {
    let mut manifests = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if manifests.len() >= GEMINI_EXTENSION_SCAN_LIMIT {
            break;
        }
        if root.join("gemini-extension.json").is_file() {
            if let Some(manifest) = load_extension_manifest(root, root.parent(), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if manifests.len() >= GEMINI_EXTENSION_SCAN_LIMIT {
                break;
            }
            let directory = entry.path();
            if !directory.is_dir() || !directory.join("gemini-extension.json").is_file() {
                continue;
            }
            if let Some(manifest) = load_extension_manifest(&directory, Some(root), cwd)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
        }
    }
    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    manifests
}

fn load_extension_manifest(
    directory: &Path,
    root: Option<&Path>,
    cwd: Option<&Path>,
) -> Option<GeminiExtension> {
    let text = read_text_limited(
        &directory.join("gemini-extension.json"),
        GEMINI_COMPAT_FILE_LIMIT,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })?;
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.parent().unwrap_or(directory).to_path_buf());
    if !extension_is_enabled(&name, cwd, &root) {
        return None;
    }
    Some(GeminiExtension {
        directory: directory.to_path_buf(),
        name,
        value,
    })
}

fn extension_is_enabled(name: &str, cwd: Option<&Path>, extension_root: &Path) -> bool {
    if let Some(enabled) = extension_name_override(name) {
        return enabled;
    }
    let Some(cwd) = cwd else {
        return true;
    };
    let path = extension_root.join("extension-enablement.json");
    let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    let Some(overrides) = value
        .get(name)
        .and_then(|extension| extension.get("overrides"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    let mut enabled = true;
    for rule in overrides.iter().filter_map(serde_json::Value::as_str) {
        if let Some(disable) = extension_override_matches(rule, cwd) {
            enabled = !disable;
        }
    }
    enabled
}

fn extension_name_override(name: &str) -> Option<bool> {
    let value = env::var("PRODEX_GEMINI_EXTENSIONS").ok()?;
    let requested = value
        .split([',', ';', ' ', '\n', '\t'])
        .filter_map(|item| {
            let item = item.trim().to_ascii_lowercase();
            (!item.is_empty()).then_some(item)
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return None;
    }
    if requested.len() == 1 && requested[0] == "none" {
        return Some(false);
    }
    Some(
        requested
            .iter()
            .any(|item| item == &name.to_ascii_lowercase()),
    )
}

fn extension_override_matches(rule: &str, cwd: &Path) -> Option<bool> {
    let mut rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    let disable = rule.starts_with('!');
    if disable {
        rule = &rule[1..];
    }
    let include_subdirs = rule.ends_with('*');
    if include_subdirs {
        rule = &rule[..rule.len().saturating_sub(1)];
    }
    let rule = normalize_enablement_path(rule);
    let cwd = normalize_enablement_path(&cwd.to_string_lossy());
    let matches = if include_subdirs {
        cwd.starts_with(&rule)
    } else {
        cwd == rule
    };
    matches.then_some(disable)
}

fn normalize_enablement_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}

fn settings_hook_sources(settings: &GeminiSettingsSource) -> Vec<serde_json::Value> {
    let mut values = Vec::new();
    if let Some(hooks) = settings.value.get("hooks") {
        values.push(hooks.clone());
    }
    for relative in [
        PathBuf::from("hooks.json"),
        Path::new("hooks").join("hooks.json"),
    ] {
        let path = settings.directory.join(relative);
        let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            values.push(value);
        }
    }
    values
}

fn gemini_mcp_settings_filters(
    settings_sources: &[GeminiSettingsSource],
) -> (Option<BTreeSet<String>>, BTreeSet<String>) {
    let mut allowed = BTreeSet::new();
    let mut excluded = BTreeSet::new();
    for source in settings_sources {
        insert_mcp_filter_values(source.value.pointer("/mcp/allowed"), &mut allowed);
        insert_mcp_filter_values(source.value.pointer("/mcp/excluded"), &mut excluded);
    }
    let allowed = (!allowed.is_empty()).then_some(allowed);
    (allowed, excluded)
}

fn insert_mcp_filter_values(value: Option<&serde_json::Value>, output: &mut BTreeSet<String>) {
    for name in json_string_array(value).unwrap_or_default() {
        let name = name.trim().to_ascii_lowercase();
        if !name.is_empty() {
            output.insert(name);
        }
    }
}

fn gemini_mcp_server_enabled_by_filters(
    server_name: &str,
    allowed: Option<&BTreeSet<String>>,
    excluded: &BTreeSet<String>,
) -> bool {
    let name = server_name.trim().to_ascii_lowercase();
    if name.is_empty() || excluded.contains(&name) {
        return false;
    }
    if let Some(allowed) = allowed
        && !allowed.contains(&name)
    {
        return false;
    }
    true
}

fn read_toml_table(path: &Path) -> Result<toml::Table> {
    let contents = fs::read_to_string(path).unwrap_or_default();
    if contents.trim().is_empty() {
        return Ok(toml::Table::new());
    }
    match toml::from_str::<toml::Value>(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?
    {
        toml::Value::Table(table) => Ok(table),
        _ => bail!("{} did not parse as a TOML table", path.display()),
    }
}

fn write_toml_table(path: &Path, table: toml::Table, context: &str) -> Result<()> {
    let rendered = toml::to_string(&toml::Value::Table(table))
        .with_context(|| format!("failed to render {context}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, rendered).with_context(|| format!("failed to write {}", path.display()))
}

fn remove_generated_mcp_servers(table: &mut toml::Table) {
    let Some(mcp_servers) = table
        .get_mut("mcp_servers")
        .and_then(toml::Value::as_table_mut)
    else {
        return;
    };
    mcp_servers.retain(|_, value| {
        value
            .as_table()
            .is_none_or(|server| server.get(GENERATED_MARKER).is_none())
    });
}

fn enable_codex_hooks_feature(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let mut table = read_toml_table(&config_path)?;
    if let Some(features) = ensure_child_table(&mut table, "features") {
        features
            .entry("hooks".to_string())
            .or_insert(toml::Value::Boolean(true));
    }
    write_toml_table(&config_path, table, "Gemini hooks feature config")
}

fn read_hooks_json(path: &Path) -> Result<serde_json::Value> {
    let contents = fs::read_to_string(path).unwrap_or_default();
    if contents.trim().is_empty() {
        return Ok(json!({"hooks": {}}));
    }
    let mut value = serde_json::from_str::<serde_json::Value>(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if !value.is_object() {
        value = json!({"hooks": {}});
    }
    if value.get("hooks").is_none() {
        value
            .as_object_mut()
            .expect("hooks root should be object")
            .insert("hooks".to_string(), json!({}));
    }
    Ok(value)
}

fn remove_generated_hooks(root: &mut serde_json::Value) {
    let Some(hooks) = root
        .get_mut("hooks")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        groups.retain(|group| {
            !group
                .get("hooks")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|hooks| hooks.iter().any(generated_command_hook))
        });
    }
}

fn generated_command_hook(hook: &serde_json::Value) -> bool {
    hook.get("statusMessage")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status.starts_with("Gemini extension "))
}

fn remove_generated_prompt_files(prompts_dir: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(prompts_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(text) = read_text_limited(&path, 4096) else {
            continue;
        };
        if text.contains(GENERATED_PROMPT_MARKER) {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn existing_prompt_slugs(prompts_dir: &Path) -> BTreeSet<String> {
    let mut slugs = BTreeSet::new();
    let Ok(entries) = fs::read_dir(prompts_dir) else {
        return slugs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            slugs.insert(safe_slug(stem));
        }
    }
    slugs
}

fn remove_generated_skill_dirs(skills_root: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(skills_root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(GENERATED_SKILL_MARKER_FILE).is_file() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn remove_generated_agent_files(agents_root: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(agents_root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        let Some(text) = read_text_limited(&path, 4096) else {
            continue;
        };
        if text.starts_with(&format!("# {GENERATED_MARKER}")) {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn read_extension_env(directory: &Path) -> BTreeMap<String, String> {
    let Some(text) = read_text_limited(&directory.join(".env"), GEMINI_COMPAT_FILE_LIMIT) else {
        return BTreeMap::new();
    };
    parse_env_file(&text)
}

fn parse_env_file(text: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for line in text.lines() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            continue;
        }
        env.insert(key.to_string(), unquote_env_value(value.trim()));
    }
    env
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn insert_env_map(
    table: &mut toml::Table,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    for (key, value) in object {
        if let Some(value) = value.as_str() {
            table.insert(key.clone(), toml::Value::String(vars.expand(value)));
        }
    }
}

fn insert_string_map(
    table: &mut toml::Table,
    key: &str,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return;
    };
    let mut output = toml::Table::new();
    for (key, value) in object {
        if let Some(value) = value.as_str() {
            output.insert(key.clone(), toml::Value::String(vars.expand(value)));
        }
    }
    if !output.is_empty() {
        table.insert(key.to_string(), toml::Value::Table(output));
    }
}

fn insert_optional_string(
    table: &mut toml::Table,
    key: &str,
    value: Option<&str>,
    vars: &GeminiCompatVars,
) {
    if let Some(value) = value {
        table.insert(key.to_string(), toml::Value::String(vars.expand(value)));
    }
}

fn insert_optional_number(table: &mut toml::Table, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        table.insert(key.to_string(), toml::Value::Integer(value as i64));
    }
}

fn insert_optional_string_array(
    table: &mut toml::Table,
    key: &str,
    value: Option<&serde_json::Value>,
    vars: &GeminiCompatVars,
) {
    if let Some(items) = json_string_array(value) {
        table.insert(
            key.to_string(),
            toml::Value::Array(
                items
                    .into_iter()
                    .map(|item| toml::Value::String(vars.expand(&item)))
                    .collect(),
            ),
        );
    }
}

fn ensure_child_table<'a>(table: &'a mut toml::Table, key: &str) -> Option<&'a mut toml::Table> {
    if !table.contains_key(key) {
        table.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    match table.get_mut(key) {
        Some(toml::Value::Table(table)) => Some(table),
        _ => None,
    }
}

fn json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn json_bool(object: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_bool))
}

fn json_u64(object: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_u64))
}

fn gemini_mcp_timeout_seconds(
    object: &serde_json::Map<String, serde_json::Value>,
    explicit_second_keys: &[&str],
) -> Option<u64> {
    json_u64(object, explicit_second_keys)
        .filter(|seconds| *seconds > 0)
        .or_else(|| json_u64(object, &["timeout"]).and_then(gemini_mcp_timeout_millis_to_seconds))
}

fn gemini_mcp_timeout_millis_to_seconds(millis: u64) -> Option<u64> {
    if millis == 0 {
        return None;
    }
    Some(millis.saturating_add(999) / 1_000).filter(|seconds| *seconds > 0)
}

fn json_string_array(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    match value? {
        serde_json::Value::String(text) => Some(vec![text.to_string()]),
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            (!values.is_empty()).then_some(values)
        }
        _ => None,
    }
}

fn toml_string(value: &toml::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(toml::Value::as_str))
        .map(str::to_string)
}

fn toml_string_array(value: &toml::Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .map(|value| match value {
            toml::Value::String(text) => vec![text.to_string()],
            toml::Value::Array(items) => items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
}

fn placeholder_mentioned(value: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    let needle = format!("${{{key}}}");
    value
        .values()
        .any(|value| value.to_string().contains(&needle))
}

fn collect_files(directory: &Path, extension: &str, limit: usize) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(directory, extension, limit, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(
    directory: &Path,
    extension: &str,
    limit: usize,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if files.len() >= limit {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if files.len() >= limit {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_files_inner(&path, extension, limit, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path);
        }
    }
    Ok(())
}

fn copy_dir_limited(source: &Path, target: &Path, limit: usize) -> Result<()> {
    let Ok(entries) = fs::read_dir(source) else {
        return Ok(());
    };
    for entry in entries.flatten().take(limit) {
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            fs::create_dir_all(&target_path)
                .with_context(|| format!("failed to create {}", target_path.display()))?;
            copy_dir_limited(&source_path, &target_path, limit)?;
        } else if source_path.is_file() {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn read_text_limited(path: &Path, limit: usize) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() as usize > limit {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_separator = false;
    for ch in value.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_separator = false;
        } else if !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "item".to_string()
    } else {
        slug
    }
}

fn safe_key(value: &str) -> String {
    safe_slug(value).replace('-', "_")
}

fn unique_slug(slug: &str, seen: &mut BTreeSet<String>) -> String {
    let base = safe_slug(slug);
    if seen.insert(base.clone()) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

fn yaml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_multiline_string_literal(value: &str) -> String {
    format!(
        "\"\"\"\n{}\n\"\"\"",
        value.replace("\"\"\"", "\\\"\\\"\\\"")
    )
}

fn first_nonempty_line(text: &str) -> Option<String> {
    strip_front_matter(text)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_matches('"').to_string())
}

fn write_executable_script(path: &Path, script: &str) -> Result<()> {
    fs::write(path, script).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn translate_gemini_prompt_placeholders(input: &str) -> String {
    let mut output = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            output.push_str(&rest[start..]);
            return output;
        };
        let key = after_start[..end].trim();
        match key {
            "args" | "arguments" => output.push_str("$ARGUMENTS"),
            _ if key.starts_with("args.") => {
                output.push('$');
                output.push_str(&safe_placeholder_name(&key[5..]));
            }
            _ => {
                output.push_str("{{");
                output.push_str(key);
                output.push_str("}}");
            }
        }
        rest = &after_start[end + 2..];
    }
    output.push_str(rest);
    output
}

fn safe_placeholder_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        "ARGUMENTS".to_string()
    } else {
        name
    }
}

fn strip_front_matter(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return text.to_string();
    }
    for line in lines.by_ref() {
        if line == "---" {
            return lines.collect::<Vec<_>>().join("\n");
        }
    }
    text.to_string()
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            output.push(path);
        }
    }
    output
}

fn gemini_env_bool(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

struct GeminiCompatVars {
    extension_path: String,
    workspace_path: String,
    separator: String,
    env: BTreeMap<String, String>,
}

impl GeminiCompatVars {
    fn new(extension_path: &Path, cwd: Option<&Path>) -> Self {
        Self {
            extension_path: extension_path.display().to_string(),
            workspace_path: cwd
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            separator: std::path::MAIN_SEPARATOR.to_string(),
            env: BTreeMap::new(),
        }
    }

    fn with_env(mut self, env: &BTreeMap<String, String>) -> Self {
        self.env = env.clone();
        self
    }

    fn expand(&self, value: &str) -> String {
        let mut value = value
            .replace("${extensionPath}", &self.extension_path)
            .replace("${extension_path}", &self.extension_path)
            .replace("${workspacePath}", &self.workspace_path)
            .replace("${workspaceRoot}", &self.workspace_path)
            .replace("${workspace_path}", &self.workspace_path)
            .replace("${cwd}", &self.workspace_path)
            .replace("${/}", &self.separator)
            .replace("${pathSeparator}", &self.separator);
        for (key, replacement) in &self.env {
            value = value.replace(&format!("${{{key}}}"), replacement);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            serde_json::from_str(&fs::read_to_string(codex_home.join("hooks.json")).unwrap())
                .unwrap();
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
            serde_json::Value::String(
                "Gemini extension workspace-tools: Checking shell".to_string()
            )
        );

        let prompt =
            fs::read_to_string(codex_home.join("prompts").join("workspace-tools-review.md"))
                .unwrap();
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
            serde_json::from_str(&fs::read_to_string(codex_home.join("hooks.json")).unwrap())
                .unwrap();
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
        let _env_lock = crate::TestEnvVarGuard::lock();
        let _home_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_HOME");
        let _system_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
        let _defaults_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
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
        let _env_lock = crate::TestEnvVarGuard::lock();
        let _home_guard = crate::TestEnvVarGuard::set("GEMINI_CLI_HOME", "/tmp/gemini-cli-home");
        let _system_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
        let _defaults_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");

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
}
