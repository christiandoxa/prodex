use super::super::log_throughput::OutputThroughput;
use super::log_stream::{LogStreamItem, collect_runtime_log_line};
use crate::{
    AppPaths, RuntimeConfig, load_runtime_broker_registry, probe_runtime_broker_log_snapshot,
    runtime_broker_registry_keys, runtime_live_log_source_registry_keys,
    runtime_process_absence_proven, runtime_process_birth_identity,
    runtime_process_executable_path, runtime_process_pid_alive,
    runtime_process_prodex_binary_identity,
};
use anyhow::Result;
use reqwest::blocking::Client;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(crate) struct LiveRuntimeLogSource {
    paths: AppPaths,
    client: Client,
    cursors: BTreeMap<String, (String, u64)>,
    verified_source_identities: BTreeMap<(String, String), bool>,
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
            verified_source_identities: BTreeMap::new(),
        })
    }

    #[cfg(test)]
    fn with_paths(paths: AppPaths, client: Client) -> Self {
        Self {
            paths,
            client,
            cursors: BTreeMap::new(),
            verified_source_identities: BTreeMap::new(),
        }
    }

    pub(crate) fn poll(&mut self) -> Vec<(PathBuf, String)> {
        let direct_keys = runtime_live_log_source_registry_keys(&self.paths)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut broker_keys = runtime_broker_registry_keys(&self.paths);
        broker_keys.extend(direct_keys.iter().cloned());
        broker_keys.sort();
        broker_keys.dedup();
        self.cursors
            .retain(|key, _| broker_keys.binary_search(key).is_ok());
        self.verified_source_identities
            .retain(|(key, _), _| broker_keys.binary_search(key).is_ok());
        let mut lines = Vec::new();
        for broker_key in broker_keys {
            let Ok(Some(registry)) = load_runtime_broker_registry(&self.paths, &broker_key) else {
                continue;
            };
            if !runtime_live_source_identity_is_valid(&registry) {
                continue;
            }
            let source_identity_key = (broker_key.clone(), registry.instance_id.clone());
            let identity_valid = *self
                .verified_source_identities
                .entry(source_identity_key)
                .or_insert_with(|| runtime_live_source_binary_identity_is_valid(&registry));
            if !identity_valid {
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
            self.cursors.insert(
                broker_key.clone(),
                (registry.instance_id.clone(), snapshot.cursor),
            );
            let source_kind = if direct_keys.contains(&broker_key) {
                "direct"
            } else {
                "broker"
            };
            let source_path = PathBuf::from(format!(
                "{source_kind}:{broker_key}:{}",
                registry.instance_id
            ));
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

fn runtime_live_source_identity_is_valid(
    registry: &prodex_runtime_broker::RuntimeBrokerRegistry,
) -> bool {
    let Some(expected_path) = registry.executable_path.as_deref() else {
        return false;
    };
    runtime_process_pid_alive(registry.pid)
        && !runtime_process_absence_proven(registry.pid)
        && registry
            .process_birth_identity
            .as_deref()
            .zip(runtime_process_birth_identity(registry.pid).as_deref())
            .is_some_and(|(expected, actual)| expected == actual)
        && runtime_process_executable_path(registry.pid)
            .as_deref()
            .is_some_and(|actual| {
                prodex_core::same_path(std::path::Path::new(expected_path), actual)
            })
}

fn runtime_live_source_binary_identity_is_valid(
    registry: &prodex_runtime_broker::RuntimeBrokerRegistry,
) -> bool {
    let observed = runtime_process_prodex_binary_identity(registry.pid);
    runtime_live_source_binary_identity_matches(registry, &observed)
}

fn runtime_live_source_binary_identity_matches(
    registry: &prodex_runtime_broker::RuntimeBrokerRegistry,
    observed: &prodex_runtime_broker::RuntimeProdexBinaryIdentity,
) -> bool {
    let Some(expected_path) = registry.executable_path.as_deref() else {
        return false;
    };
    let expected = prodex_runtime_broker::runtime_registry_prodex_binary_identity(registry);
    expected.is_present()
        && observed.is_present()
        && prodex_runtime_broker::runtime_prodex_binary_identity_matches(&expected, observed)
        && observed.executable_path.as_deref().is_some_and(|actual| {
            prodex_core::same_path(std::path::Path::new(expected_path), actual)
        })
}

pub(crate) fn collect_live_log_items(
    live_source: &mut Option<LiveRuntimeLogSource>,
    include_operational_insights: bool,
    mut throughput: Option<&mut OutputThroughput>,
) -> Result<Vec<LogStreamItem>> {
    let Some(live_source) = live_source.as_mut() else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for (path, line) in live_source.poll() {
        items.extend(collect_runtime_log_line(
            &path,
            &line,
            include_operational_insights,
            throughput.as_deref_mut(),
            true,
        )?);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RUNTIME_PROXY_OPENAI_MOUNT_PATH, RuntimeBrokerRegistry,
        runtime_current_prodex_binary_identity, runtime_process_birth_identity,
        save_runtime_broker_artifacts,
    };
    use prodex_runtime_broker::RuntimeBrokerSecret;
    use reqwest::blocking::Client;
    use std::fs;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tiny_http::{Header, Response, Server};

    fn test_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "prodex-live-log-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
            root,
        }
    }

    fn test_registry(key: &str, instance_id: &str, listen_addr: &str) -> RuntimeBrokerRegistry {
        let identity = runtime_current_prodex_binary_identity();
        RuntimeBrokerRegistry {
            pid: std::process::id(),
            process_birth_identity: runtime_process_birth_identity(std::process::id()),
            listen_addr: listen_addr.to_string(),
            started_at: 100,
            upstream_base_url: "https://upstream.example".to_string(),
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: key.starts_with("live-"),
            current_profile: "main".to_string(),
            instance_id: instance_id.to_string(),
            prodex_version: identity.prodex_version,
            executable_path: identity
                .executable_path
                .map(|path| path.display().to_string()),
            executable_sha256: identity.executable_sha256,
            openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
            realtime_ws_addr: None,
        }
    }

    fn snapshot_server(line: &'static str) -> (String, thread::JoinHandle<()>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            let body = serde_json::json!({
                "cursor": 1,
                "dropped": 0,
                "entries": [{"sequence": 1, "line": line}],
            });
            let response = Response::from_string(body.to_string())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            request.respond(response).unwrap();
        });
        (address.to_string(), handle)
    }

    #[test]
    fn direct_and_broker_sources_reach_throughput_without_disk_logs() {
        let paths = test_paths("coexist");
        let (direct_addr, direct_server) = snapshot_server(
            "[2026-09-03 10:00:00.000 +00:00] token_usage request=1 transport=http profile=main source=responses_sse input_tokens=10 output_tokens=42 generation_ms=1000 output_tokens_per_second=42.0\n",
        );
        let (broker_addr, broker_server) = snapshot_server(
            "[2026-09-03 10:00:01.000 +00:00] token_usage request=2 transport=http profile=other source=responses_sse input_tokens=10 output_tokens=9 generation_ms=1000 output_tokens_per_second=9.0\n",
        );
        for (key, instance, address, token) in [
            (
                "live-direct",
                "direct-instance",
                direct_addr,
                "direct-capability",
            ),
            (
                "broker",
                "broker-instance",
                broker_addr,
                "broker-capability",
            ),
        ] {
            let registry = test_registry(key, instance, &address);
            let secret = RuntimeBrokerSecret::new(token).unwrap();
            save_runtime_broker_artifacts(&paths, key, instance, &secret, &registry).unwrap();
        }

        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        let mut source = LiveRuntimeLogSource::with_paths(paths.clone(), client);
        source.verified_source_identities.insert(
            ("live-direct".to_string(), "direct-instance".to_string()),
            true,
        );
        source
            .verified_source_identities
            .insert(("broker".to_string(), "broker-instance".to_string()), true);
        let mut live_source = Some(source);
        let mut throughput = crate::app_commands::log_throughput::OutputThroughput::default();
        let items = collect_live_log_items(&mut live_source, true, Some(&mut throughput)).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(
            throughput.display_rate_for_profile(std::time::Instant::now(), Some("main")),
            Some(42.0)
        );
        assert_eq!(
            throughput.display_rate_for_profile(std::time::Instant::now(), Some("main")),
            Some(42.0)
        );

        direct_server.join().unwrap();
        broker_server.join().unwrap();
        fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn stale_pid_birth_and_binary_mismatch_sources_are_rejected_before_http() {
        let paths = test_paths("identity");
        let valid = test_registry("broker", "instance", "127.0.0.1:1");
        assert!(runtime_live_source_identity_is_valid(&valid));
        assert!(runtime_live_source_binary_identity_matches(
            &valid,
            &runtime_current_prodex_binary_identity()
        ));

        let mut stale = valid.clone();
        stale.pid = u32::MAX;
        assert!(!runtime_live_source_identity_is_valid(&stale));

        let mut reused = valid.clone();
        reused.process_birth_identity = Some("different-birth".to_string());
        assert!(!runtime_live_source_identity_is_valid(&reused));

        let mut other_binary = valid;
        other_binary.executable_path = Some("/opt/other/prodex".to_string());
        assert!(!runtime_live_source_binary_identity_matches(
            &other_binary,
            &runtime_current_prodex_binary_identity()
        ));

        fs::remove_dir_all(paths.root).unwrap();
    }
}
