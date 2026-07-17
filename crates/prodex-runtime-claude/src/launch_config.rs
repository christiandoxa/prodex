use crate::constants::PRODEX_CLAUDE_DEFAULT_WEB_TOOLS;
use crate::paths::{runtime_proxy_claude_config_path, runtime_proxy_claude_settings_path};
use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn ensure_runtime_proxy_claude_launch_config(
    config_dir: &Path,
    cwd: &Path,
    claude_version: Option<&str>,
) -> Result<()> {
    fs::create_dir_all(config_dir).with_context(|| {
        format!(
            "failed to create Claude Code config dir at {}",
            config_dir.display()
        )
    })?;
    let _lock = acquire_runtime_proxy_claude_config_lock(config_dir)?;
    let config_path = runtime_proxy_claude_config_path(config_dir);
    let mut config = load_runtime_proxy_claude_json_object(&config_path, "config")?;
    if !config.is_object() {
        anyhow::bail!(
            "Claude Code config at {} must be a JSON object",
            config_path.display()
        );
    }

    let object = config
        .as_object_mut()
        .expect("Claude Code config should be normalized to an object");
    object.remove("skipWebFetchPreflight");
    let num_startups = object
        .get("numStartups")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .max(1);
    object.insert("numStartups".to_string(), serde_json::json!(num_startups));
    object.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::json!(true),
    );
    if let Some(version) = claude_version {
        object.insert(
            "lastOnboardingVersion".to_string(),
            serde_json::json!(version),
        );
    }
    let mut additional_model_options =
        runtime_anthropic_crate::runtime_proxy_claude_additional_model_option_entries();
    if let Some(existing) = object
        .get("additionalModelOptionsCache")
        .and_then(serde_json::Value::as_array)
    {
        for entry in existing {
            let existing_value = entry.get("value").and_then(serde_json::Value::as_str);
            if existing_value.is_some_and(
                runtime_anthropic_crate::runtime_proxy_claude_managed_model_option_value,
            ) {
                continue;
            }
            additional_model_options.push(entry.clone());
        }
    }
    object.insert(
        "additionalModelOptionsCache".to_string(),
        serde_json::Value::Array(additional_model_options),
    );

    let projects = object
        .entry("projects".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !projects.is_object() {
        *projects = serde_json::json!({});
    }
    let projects = projects
        .as_object_mut()
        .expect("Claude Code projects config should be an object");
    let project_key = cwd.to_string_lossy().into_owned();
    let project = projects
        .entry(project_key)
        .or_insert_with(|| serde_json::json!({}));
    if !project.is_object() {
        *project = serde_json::json!({});
    }
    let project = project
        .as_object_mut()
        .expect("Claude Code project config should be an object");
    project.insert(
        "hasTrustDialogAccepted".to_string(),
        serde_json::json!(true),
    );
    let project_onboarding_seen_count = project
        .get("projectOnboardingSeenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .max(1);
    project.insert(
        "projectOnboardingSeenCount".to_string(),
        serde_json::json!(project_onboarding_seen_count),
    );
    for key in [
        "allowedTools",
        "mcpContextUris",
        "enabledMcpjsonServers",
        "disabledMcpjsonServers",
        "exampleFiles",
    ] {
        if !project.get(key).is_some_and(serde_json::Value::is_array) {
            project.insert(key.to_string(), serde_json::json!([]));
        }
    }
    if let Some(allowed_tools) = project
        .get_mut("allowedTools")
        .and_then(serde_json::Value::as_array_mut)
    {
        append_default_web_tools(allowed_tools);
    }
    if !project
        .get("mcpServers")
        .is_some_and(serde_json::Value::is_object)
    {
        project.insert("mcpServers".to_string(), serde_json::json!({}));
    }
    project.insert(
        "hasClaudeMdExternalIncludesApproved".to_string(),
        serde_json::json!(
            project
                .get("hasClaudeMdExternalIncludesApproved")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        ),
    );
    project.insert(
        "hasClaudeMdExternalIncludesWarningShown".to_string(),
        serde_json::json!(
            project
                .get("hasClaudeMdExternalIncludesWarningShown")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        ),
    );

    let rendered =
        serde_json::to_string_pretty(&config).context("failed to render Claude Code config")?;
    write_runtime_proxy_claude_json_atomic(&config_path, rendered.as_bytes())?;
    ensure_runtime_proxy_claude_settings_unlocked(config_dir)?;
    Ok(())
}

pub fn ensure_runtime_proxy_claude_settings(config_dir: &Path) -> Result<()> {
    fs::create_dir_all(config_dir).with_context(|| {
        format!(
            "failed to create Claude Code config dir at {}",
            config_dir.display()
        )
    })?;
    let _lock = acquire_runtime_proxy_claude_config_lock(config_dir)?;
    ensure_runtime_proxy_claude_settings_unlocked(config_dir)
}

fn ensure_runtime_proxy_claude_settings_unlocked(config_dir: &Path) -> Result<()> {
    let settings_path = runtime_proxy_claude_settings_path(config_dir);
    let mut settings = load_runtime_proxy_claude_json_object(&settings_path, "settings")?;
    if !settings.is_object() {
        anyhow::bail!(
            "Claude Code settings at {} must be a JSON object",
            settings_path.display()
        );
    }

    let object = settings
        .as_object_mut()
        .expect("Claude Code settings should be normalized to an object");
    object.insert("skipWebFetchPreflight".to_string(), serde_json::json!(true));
    let permissions = object
        .entry("permissions".to_string())
        .or_insert_with(|| serde_json::json!({}));
    if !permissions.is_object() {
        *permissions = serde_json::json!({});
    }
    let permissions = permissions
        .as_object_mut()
        .expect("Claude Code permissions should be normalized to an object");
    let allow = permissions
        .entry("allow".to_string())
        .or_insert_with(|| serde_json::json!([]));
    if !allow.is_array() {
        *allow = serde_json::json!([]);
    }
    if let Some(allow) = allow.as_array_mut() {
        append_default_web_tools(allow);
    }

    let rendered =
        serde_json::to_string_pretty(&settings).context("failed to render Claude Code settings")?;
    write_runtime_proxy_claude_json_atomic(&settings_path, rendered.as_bytes())?;
    Ok(())
}

fn load_runtime_proxy_claude_json_object(path: &Path, label: &str) -> Result<serde_json::Value> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(serde_json::json!({})),
        Err(err) => {
            return Err(err).with_context(|| {
                format!("failed to read Claude Code {label} at {}", path.display())
            });
        }
    };
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse Claude Code {label} at {}", path.display()))
}

fn acquire_runtime_proxy_claude_config_lock(config_dir: &Path) -> Result<File> {
    let path = config_dir.join(".prodex-config.lock");
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| {
            format!(
                "failed to open Claude Code config lock at {}",
                path.display()
            )
        })?;
    file.lock_exclusive()
        .with_context(|| format!("failed to lock Claude Code config at {}", path.display()))?;
    Ok(file)
}

fn write_runtime_proxy_claude_json_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let parent = path
        .parent()
        .context("Claude Code config path has no parent")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Claude Code config path has no UTF-8 file name")?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp_path)
            .with_context(|| format!("failed to create {}", temp_path.display()))?;
        file.write_all(contents)
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        fs::rename(&temp_path, path).with_context(|| {
            format!("failed to replace Claude Code config at {}", path.display())
        })?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("failed to sync {}", parent.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn append_default_web_tools(tools: &mut Vec<serde_json::Value>) {
    let mut seen = tools
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for tool in PRODEX_CLAUDE_DEFAULT_WEB_TOOLS {
        if seen.insert((*tool).to_string()) {
            tools.push(serde_json::Value::String((*tool).to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "prodex-claude-config-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn malformed_claude_config_is_rejected_without_overwrite() {
        let root = TestDir::new("malformed");
        let config_path = runtime_proxy_claude_config_path(&root.0);
        fs::write(&config_path, "{not-json").unwrap();

        assert!(
            ensure_runtime_proxy_claude_launch_config(&root.0, Path::new("/tmp/a"), None).is_err()
        );
        assert_eq!(fs::read_to_string(config_path).unwrap(), "{not-json");
    }

    #[test]
    fn concurrent_claude_config_updates_preserve_both_projects() {
        let root = TestDir::new("concurrent");
        let barrier = Arc::new(Barrier::new(3));
        let handles = ["project-a", "project-b"].map(|project| {
            let config_dir = root.0.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                ensure_runtime_proxy_claude_launch_config(&config_dir, Path::new(project), None)
                    .unwrap();
            })
        });
        barrier.wait();
        for handle in handles {
            handle.join().unwrap();
        }

        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(runtime_proxy_claude_config_path(&root.0)).unwrap())
                .unwrap();
        assert!(config.pointer("/projects/project-a").is_some());
        assert!(config.pointer("/projects/project-b").is_some());
    }
}
