use super::fs_utils::read_text_limited;
use super::utils::GeminiCompatVars;
use super::{
    GEMINI_COMPAT_FILE_LIMIT, GeminiExtension, ensure_child_table, read_toml_table,
    write_toml_table,
};
use crate::{GeminiSettingsSource, gemini_settings_sources};
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn write_gemini_hooks(
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
