use super::*;

pub(crate) fn load_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(paths, broker_key);
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    match load_json_file_with_backup::<RuntimeBrokerRegistry>(&path, &backup_path) {
        Ok(loaded) => Ok(Some(loaded.value)),
        Err(_err) if !path.exists() && !backup_path.exists() => Ok(None),
        Err(err) => Err(err),
    }
}

pub(crate) fn save_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<()> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize runtime broker registry")?;
    write_json_file_with_backup(
        &path,
        &runtime_broker_registry_last_good_file_path(paths, broker_key),
        &json,
        |content| {
            let _: RuntimeBrokerRegistry = serde_json::from_str(content)
                .context("failed to validate runtime broker registry")?;
            Ok(())
        },
    )
}

pub(crate) fn remove_runtime_broker_registry_if_token_matches(
    paths: &AppPaths,
    broker_key: &str,
    instance_token: &str,
) {
    let Ok(Some(existing)) = load_runtime_broker_registry(paths, broker_key) else {
        return;
    };
    if existing.instance_token != instance_token {
        return;
    }
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        let _ = fs::remove_file(path);
    }
}
