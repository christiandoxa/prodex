use crate::{
    AppPaths, RuntimeConfig, load_runtime_broker_registry, probe_runtime_broker_log_snapshot,
    runtime_broker_registry_keys, runtime_process_pid_alive,
};
use reqwest::blocking::Client;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) struct LiveRuntimeLogSource {
    paths: AppPaths,
    client: Client,
    cursors: BTreeMap<String, (String, u64)>,
}

impl LiveRuntimeLogSource {
    pub(crate) fn new() -> Option<Self> {
        let paths = AppPaths::discover().ok()?;
        let config = RuntimeConfig::from_env_policy_and_cli(&paths).ok()?;
        let client = crate::runtime_broker_client_with_config(&config).ok()?;
        Some(Self {
            paths,
            client,
            cursors: BTreeMap::new(),
        })
    }

    pub(crate) fn poll(&mut self) -> Vec<(PathBuf, String)> {
        let mut lines = Vec::new();
        for broker_key in runtime_broker_registry_keys(&self.paths) {
            let Ok(Some(registry)) = load_runtime_broker_registry(&self.paths, &broker_key) else {
                continue;
            };
            if !runtime_process_pid_alive(registry.pid) {
                continue;
            }
            let cursor = self
                .cursors
                .get(&broker_key)
                .filter(|(instance_id, _)| instance_id == &registry.instance_id)
                .map(|(_, cursor)| *cursor)
                .unwrap_or(0);
            let Ok(Some(snapshot)) = probe_runtime_broker_log_snapshot(
                &self.client,
                &self.paths,
                &broker_key,
                &registry,
                cursor,
            ) else {
                continue;
            };
            self.cursors
                .insert(broker_key.clone(), (registry.instance_id, snapshot.cursor));
            let source_path = PathBuf::from(format!("broker:{broker_key}"));
            lines.extend(
                snapshot
                    .entries
                    .into_iter()
                    .map(|entry| (source_path.clone(), entry.line)),
            );
        }
        lines
    }
}
