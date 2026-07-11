use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_domain::{PolicyRevisionId, TenantId};
use prodex_observability::{
    ConfigPublicationDeliveryMetricPlan, ConfigPublicationDeliveryResult,
    ConfigPublicationDeliveryTarget, plan_config_publication_delivery_metric,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use prodex_runtime_policy::RuntimePolicyCacheInvalidationPlan;
pub use prodex_runtime_policy::clear_runtime_policy_cache;
pub(super) use prodex_runtime_policy::{
    RuntimeLogFormat, RuntimePolicySummary, ensure_runtime_policy_valid, runtime_policy_proxy,
    runtime_policy_runtime, runtime_policy_secrets, runtime_policy_summary,
};

const CONFIG_PUBLICATION_TRANSPORT_SCHEMA_VERSION: u32 = 1;
const CONFIG_PUBLICATION_TRANSPORT_DIR: &str = "config-publication";
const CONFIG_PUBLICATION_TRANSPORT_OUTBOX_DIR: &str = "outbox";
const CONFIG_PUBLICATION_TRANSPORT_ACK_DIR: &str = "acks";
static LAST_CONFIG_PUBLICATION_TIMESTAMP_MS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePolicyPublicationDeliveryPlan {
    pub root: PathBuf,
    pub gateway_cache_refreshed: bool,
    pub runtime_policy_invalidation: RuntimePolicyCacheInvalidationPlan,
    pub runtime_policy_version: Option<u32>,
    pub delivery_metrics: [ConfigPublicationDeliveryMetricPlan; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationTransportPublishPlan {
    pub transport_root: PathBuf,
    pub event_path: PathBuf,
    pub event_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationTransportDeliveryPlan {
    pub transport_root: PathBuf,
    pub replica: String,
    pub root: PathBuf,
    pub delivered_event_count: usize,
    pub runtime_policy_version: Option<u32>,
    pub delivery_metrics: [ConfigPublicationDeliveryMetricPlan; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationTransportCompactionPlan {
    pub transport_root: PathBuf,
    pub replica_count: usize,
    pub eligible_event_count: usize,
    pub removed_event_count: usize,
    pub retained_event_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ConfigPublicationTransportEventFile {
    schema_version: u32,
    event_id: String,
    published_at_unix_ms: u64,
    tenant_id: TenantId,
    activated_revision_id: PolicyRevisionId,
    previous_active_revision_id: Option<PolicyRevisionId>,
    last_known_good_revision_id: Option<PolicyRevisionId>,
    targets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ConfigPublicationTransportAckFile {
    schema_version: u32,
    event_id: String,
    delivered_at_unix_ms: u64,
}

pub fn deliver_config_publication_event_to_gateway_runtime(
    event: &ConfigPublicationEventPlan,
    root: &Path,
) -> Result<RuntimePolicyPublicationDeliveryPlan> {
    let mut gateway_cache_refresh = false;
    let mut runtime_policy_reload = false;
    for target in event.targets {
        match target {
            ConfigPublicationEventTarget::GatewayCacheRefresh => gateway_cache_refresh = true,
            ConfigPublicationEventTarget::RuntimePolicyReload => runtime_policy_reload = true,
        }
    }

    if !gateway_cache_refresh {
        bail!("configuration publication event is missing gateway cache refresh target");
    }
    if !runtime_policy_reload {
        bail!("configuration publication event is missing runtime policy reload target");
    }

    let (runtime_policy_invalidation, runtime_policy) =
        prodex_runtime_policy::reload_runtime_policy_cached_with_invalidation(root)?;
    let delivery_metrics = [
        plan_config_publication_delivery_metric(
            ConfigPublicationDeliveryTarget::GatewayCacheRefresh,
            ConfigPublicationDeliveryResult::Delivered,
        )
        .map_err(|err| {
            anyhow!("failed to plan gateway config publication delivery metric: {err:?}")
        })?,
        plan_config_publication_delivery_metric(
            ConfigPublicationDeliveryTarget::RuntimePolicyReload,
            ConfigPublicationDeliveryResult::Delivered,
        )
        .map_err(|err| {
            anyhow!("failed to plan runtime config publication delivery metric: {err:?}")
        })?,
    ];

    Ok(RuntimePolicyPublicationDeliveryPlan {
        root: root.to_path_buf(),
        gateway_cache_refreshed: true,
        runtime_policy_invalidation,
        runtime_policy_version: runtime_policy.map(|policy| policy.version),
        delivery_metrics,
    })
}

pub fn publish_config_publication_event_to_gateway_transport(
    event: &ConfigPublicationEventPlan,
    transport_root: &Path,
) -> Result<ConfigPublicationTransportPublishPlan> {
    let record = config_publication_transport_event_file(event);
    let outbox_dir = config_publication_transport_outbox_dir(transport_root);
    std::fs::create_dir_all(&outbox_dir)?;

    let event_path = outbox_dir.join(format!("{}.json", record.event_id));
    if !event_path.exists() {
        write_config_publication_transport_json(&event_path, &serde_json::to_vec_pretty(&record)?)?;
    }

    Ok(ConfigPublicationTransportPublishPlan {
        transport_root: transport_root.to_path_buf(),
        event_path,
        event_id: record.event_id,
    })
}

pub fn deliver_pending_config_publication_events_to_gateway_runtime(
    transport_root: &Path,
    replica: &str,
    root: &Path,
) -> Result<ConfigPublicationTransportDeliveryPlan> {
    validate_config_publication_transport_replica(replica)?;
    let ack_dir = config_publication_transport_ack_dir(transport_root, replica);
    std::fs::create_dir_all(&ack_dir)?;

    let mut records = read_config_publication_transport_event_files(transport_root)?;
    records.sort_by(|left, right| {
        left.published_at_unix_ms
            .cmp(&right.published_at_unix_ms)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });

    let mut delivered_event_count = 0usize;
    let mut runtime_policy_version = None;
    for record in records {
        let ack_path = ack_dir.join(format!("{}.ack.json", record.event_id));
        if ack_path.exists() {
            continue;
        }
        let delivery =
            deliver_config_publication_event_to_gateway_runtime(&record.to_event_plan()?, root)?;
        runtime_policy_version = delivery.runtime_policy_version;
        delivered_event_count += 1;
        let ack = ConfigPublicationTransportAckFile {
            schema_version: CONFIG_PUBLICATION_TRANSPORT_SCHEMA_VERSION,
            event_id: record.event_id,
            delivered_at_unix_ms: unix_time_now_ms(),
        };
        write_config_publication_transport_json(&ack_path, &serde_json::to_vec_pretty(&ack)?)?;
    }

    Ok(ConfigPublicationTransportDeliveryPlan {
        transport_root: transport_root.to_path_buf(),
        replica: replica.to_string(),
        root: root.to_path_buf(),
        delivered_event_count,
        runtime_policy_version,
        delivery_metrics: config_publication_transport_delivery_metrics(
            delivered_event_count as u64,
        )?,
    })
}

pub fn compact_config_publication_transport(
    transport_root: &Path,
    retain: usize,
) -> Result<ConfigPublicationTransportCompactionPlan> {
    let mut records = read_config_publication_transport_event_files(transport_root)?;
    records.sort_by(|left, right| {
        left.published_at_unix_ms
            .cmp(&right.published_at_unix_ms)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let replicas = read_config_publication_transport_replica_ids(transport_root)?;
    if replicas.is_empty() {
        return Ok(ConfigPublicationTransportCompactionPlan {
            transport_root: transport_root.to_path_buf(),
            replica_count: 0,
            eligible_event_count: 0,
            removed_event_count: 0,
            retained_event_count: records.len(),
        });
    }

    let mut eligible_event_ids = records
        .iter()
        .filter(|record| {
            replicas.iter().all(|replica| {
                config_publication_transport_ack_dir(transport_root, replica)
                    .join(format!("{}.ack.json", record.event_id))
                    .exists()
            })
        })
        .map(|record| record.event_id.clone())
        .collect::<Vec<_>>();
    let eligible_event_count = eligible_event_ids.len();
    let removed_event_ids = if eligible_event_ids.len() > retain {
        let _ = eligible_event_ids.split_off(eligible_event_ids.len() - retain);
        eligible_event_ids
    } else {
        Vec::new()
    };

    for event_id in &removed_event_ids {
        let event_path = config_publication_transport_outbox_dir(transport_root)
            .join(format!("{event_id}.json"));
        if event_path.exists() {
            std::fs::remove_file(&event_path)?;
        }
        for replica in &replicas {
            let ack_path = config_publication_transport_ack_dir(transport_root, replica)
                .join(format!("{event_id}.ack.json"));
            if ack_path.exists() {
                std::fs::remove_file(&ack_path)?;
            }
        }
    }

    Ok(ConfigPublicationTransportCompactionPlan {
        transport_root: transport_root.to_path_buf(),
        replica_count: replicas.len(),
        eligible_event_count,
        removed_event_count: removed_event_ids.len(),
        retained_event_count: read_config_publication_transport_event_files(transport_root)?.len(),
    })
}

impl ConfigPublicationTransportEventFile {
    fn to_event_plan(&self) -> Result<ConfigPublicationEventPlan> {
        if self.schema_version != CONFIG_PUBLICATION_TRANSPORT_SCHEMA_VERSION {
            bail!(
                "unsupported config publication transport schema version {}",
                self.schema_version
            );
        }
        let targets = self
            .targets
            .iter()
            .map(|target| parse_config_publication_event_target(target))
            .collect::<Result<Vec<_>>>()?;
        let targets: [ConfigPublicationEventTarget; 2] = targets
            .try_into()
            .map_err(|_| anyhow!("configuration publication event is incomplete"))?;
        Ok(ConfigPublicationEventPlan {
            tenant_id: self.tenant_id,
            activated_revision_id: self.activated_revision_id,
            previous_active_revision_id: self.previous_active_revision_id,
            last_known_good_revision_id: self.last_known_good_revision_id,
            targets,
        })
    }
}

fn config_publication_transport_event_file(
    event: &ConfigPublicationEventPlan,
) -> ConfigPublicationTransportEventFile {
    let targets = event
        .targets
        .iter()
        .map(|target| config_publication_event_target_name(*target).to_string())
        .collect::<Vec<_>>();
    ConfigPublicationTransportEventFile {
        schema_version: CONFIG_PUBLICATION_TRANSPORT_SCHEMA_VERSION,
        event_id: config_publication_transport_event_id(
            event.tenant_id,
            event.activated_revision_id,
            event.previous_active_revision_id,
            event.last_known_good_revision_id,
            &targets,
        ),
        published_at_unix_ms: next_config_publication_timestamp_ms(),
        tenant_id: event.tenant_id,
        activated_revision_id: event.activated_revision_id,
        previous_active_revision_id: event.previous_active_revision_id,
        last_known_good_revision_id: event.last_known_good_revision_id,
        targets,
    }
}

fn config_publication_transport_event_id(
    tenant_id: TenantId,
    activated_revision_id: PolicyRevisionId,
    previous_active_revision_id: Option<PolicyRevisionId>,
    last_known_good_revision_id: Option<PolicyRevisionId>,
    targets: &[String],
) -> String {
    let mut hash = Sha256::new();
    hash.update(tenant_id.to_string());
    hash.update([0]);
    hash.update(activated_revision_id.to_string());
    hash.update([0]);
    hash.update(
        previous_active_revision_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    hash.update([0]);
    hash.update(
        last_known_good_revision_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    for target in targets {
        hash.update([0]);
        hash.update(target.as_bytes());
    }
    let digest = hash.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn config_publication_transport_dir(transport_root: &Path) -> PathBuf {
    transport_root.join(CONFIG_PUBLICATION_TRANSPORT_DIR)
}

fn config_publication_transport_outbox_dir(transport_root: &Path) -> PathBuf {
    config_publication_transport_dir(transport_root).join(CONFIG_PUBLICATION_TRANSPORT_OUTBOX_DIR)
}

fn config_publication_transport_ack_dir(transport_root: &Path, replica: &str) -> PathBuf {
    config_publication_transport_dir(transport_root)
        .join(CONFIG_PUBLICATION_TRANSPORT_ACK_DIR)
        .join(replica)
}

fn read_config_publication_transport_event_files(
    transport_root: &Path,
) -> Result<Vec<ConfigPublicationTransportEventFile>> {
    let outbox_dir = config_publication_transport_outbox_dir(transport_root);
    let entries = match std::fs::read_dir(&outbox_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    let mut records = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let record = serde_json::from_slice::<ConfigPublicationTransportEventFile>(&bytes)
            .map_err(|err| {
                anyhow!(
                    "failed to parse config publication transport event {}: {err}",
                    path.display()
                )
            })?;
        records.push(record);
    }
    Ok(records)
}

fn read_config_publication_transport_replica_ids(transport_root: &Path) -> Result<Vec<String>> {
    let ack_root =
        config_publication_transport_dir(transport_root).join(CONFIG_PUBLICATION_TRANSPORT_ACK_DIR);
    let entries = match std::fs::read_dir(&ack_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut replicas = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let replica = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("config publication replica id is not valid utf-8"))?;
        validate_config_publication_transport_replica(&replica)?;
        replicas.push(replica);
    }
    replicas.sort();
    Ok(replicas)
}

fn config_publication_transport_delivery_metrics(
    delivered_event_count: u64,
) -> Result<[ConfigPublicationDeliveryMetricPlan; 2]> {
    let mut gateway_cache_refresh = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::GatewayCacheRefresh,
        ConfigPublicationDeliveryResult::Delivered,
    )
    .map_err(|err| anyhow!("failed to plan gateway config publication delivery metric: {err:?}"))?;
    gateway_cache_refresh.increment = delivered_event_count;

    let mut runtime_policy_reload = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::RuntimePolicyReload,
        ConfigPublicationDeliveryResult::Delivered,
    )
    .map_err(|err| anyhow!("failed to plan runtime config publication delivery metric: {err:?}"))?;
    runtime_policy_reload.increment = delivered_event_count;

    Ok([gateway_cache_refresh, runtime_policy_reload])
}

fn write_config_publication_transport_json(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config publication transport path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("event"),
        std::process::id(),
        unix_time_now_ms()
    ));
    std::fs::write(&temp_path, bytes)?;
    match std::fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&temp_path);
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(err.into())
        }
    }
}

fn validate_config_publication_transport_replica(replica: &str) -> Result<()> {
    if replica.is_empty()
        || !replica
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("invalid config publication replica id");
    }
    Ok(())
}

fn parse_config_publication_event_target(value: &str) -> Result<ConfigPublicationEventTarget> {
    match value {
        "gateway_cache_refresh" => Ok(ConfigPublicationEventTarget::GatewayCacheRefresh),
        "runtime_policy_reload" => Ok(ConfigPublicationEventTarget::RuntimePolicyReload),
        other => bail!("unsupported configuration publication event target {other}"),
    }
}

fn config_publication_event_target_name(target: ConfigPublicationEventTarget) -> &'static str {
    match target {
        ConfigPublicationEventTarget::GatewayCacheRefresh => "gateway_cache_refresh",
        ConfigPublicationEventTarget::RuntimePolicyReload => "runtime_policy_reload",
    }
}

fn unix_time_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn next_config_publication_timestamp_ms() -> u64 {
    let now = unix_time_now_ms();
    let mut previous = LAST_CONFIG_PUBLICATION_TIMESTAMP_MS.load(Ordering::Relaxed);
    loop {
        let next = now.max(previous.saturating_add(1));
        match LAST_CONFIG_PUBLICATION_TIMESTAMP_MS.compare_exchange_weak(
            previous,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return next,
            Err(actual) => previous = actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPolicyDir {
        root: PathBuf,
    }

    impl TestPolicyDir {
        fn new() -> Self {
            clear_runtime_policy_cache();
            let root = std::env::temp_dir().join(format!(
                "prodex-config-publication-delivery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn write_policy(&self, worker_count: u32) {
            std::fs::write(
                self.root.join("policy.toml"),
                format!(
                    r#"
version = 1

[runtime_proxy]
worker_count = {worker_count}
"#,
                ),
            )
            .unwrap();
        }
    }

    impl Drop for TestPolicyDir {
        fn drop(&mut self) {
            clear_runtime_policy_cache();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    struct TestTransportDir {
        root: PathBuf,
    }

    impl TestTransportDir {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "prodex-config-publication-transport-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TestTransportDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn complete_event() -> ConfigPublicationEventPlan {
        ConfigPublicationEventPlan {
            tenant_id: TenantId::new(),
            activated_revision_id: PolicyRevisionId::new(),
            previous_active_revision_id: Some(PolicyRevisionId::new()),
            last_known_good_revision_id: None,
            targets: [
                ConfigPublicationEventTarget::GatewayCacheRefresh,
                ConfigPublicationEventTarget::RuntimePolicyReload,
            ],
        }
    }

    #[test]
    fn config_publication_event_invalidates_and_reloads_runtime_policy_cache() {
        let policy_dir = TestPolicyDir::new();
        policy_dir.write_policy(1);
        let loaded = prodex_runtime_policy::load_runtime_policy_cached(&policy_dir.root)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.runtime_proxy.worker_count, Some(1));

        policy_dir.write_policy(2);
        let delivery = deliver_config_publication_event_to_gateway_runtime(
            &complete_event(),
            &policy_dir.root,
        )
        .unwrap();

        assert_eq!(delivery.root, policy_dir.root);
        assert!(delivery.gateway_cache_refreshed);
        assert!(delivery.runtime_policy_invalidation.had_cached_entry);
        assert_eq!(
            delivery.runtime_policy_invalidation.cached_policy_version,
            Some(1)
        );
        assert_eq!(delivery.runtime_policy_version, Some(1));
        assert_eq!(delivery.delivery_metrics[0].increment, 1);
        assert_eq!(
            delivery.delivery_metrics[0]
                .target_label
                .as_metric_label()
                .unwrap(),
            ("config_publication_target", "gateway_cache_refresh")
        );
        assert_eq!(
            delivery.delivery_metrics[1]
                .target_label
                .as_metric_label()
                .unwrap(),
            ("config_publication_target", "runtime_policy_reload")
        );
        assert_eq!(
            delivery.delivery_metrics[1]
                .result_label
                .as_metric_label()
                .unwrap(),
            ("config_publication_result", "delivered")
        );
        let reloaded = prodex_runtime_policy::load_runtime_policy_cached(&policy_dir.root)
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.version, 1);
        assert_eq!(reloaded.runtime_proxy.worker_count, Some(2));
    }

    #[test]
    fn config_publication_transport_is_idempotent_and_replica_scoped() {
        let policy_dir = TestPolicyDir::new();
        policy_dir.write_policy(3);
        let transport_dir = TestTransportDir::new();
        let event = complete_event();

        let first_publish =
            publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root)
                .unwrap();
        let second_publish =
            publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root)
                .unwrap();

        assert_eq!(first_publish.event_id, second_publish.event_id);
        assert_eq!(first_publish.event_path, second_publish.event_path);

        let first_delivery = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-a",
            &policy_dir.root,
        )
        .unwrap();
        assert_eq!(first_delivery.delivered_event_count, 1);
        assert_eq!(first_delivery.runtime_policy_version, Some(1));
        assert_eq!(first_delivery.delivery_metrics[0].increment, 1);
        assert_eq!(first_delivery.delivery_metrics[1].increment, 1);

        let repeat_delivery = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-a",
            &policy_dir.root,
        )
        .unwrap();
        assert_eq!(repeat_delivery.delivered_event_count, 0);
        assert_eq!(repeat_delivery.delivery_metrics[0].increment, 0);
        assert_eq!(repeat_delivery.delivery_metrics[1].increment, 0);

        let second_replica_delivery = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-b",
            &policy_dir.root,
        )
        .unwrap();
        assert_eq!(second_replica_delivery.delivered_event_count, 1);
        assert_eq!(second_replica_delivery.delivery_metrics[0].increment, 1);
        assert_eq!(second_replica_delivery.delivery_metrics[1].increment, 1);
    }

    #[test]
    fn config_publication_transport_rejects_invalid_replica_ids() {
        let policy_dir = TestPolicyDir::new();
        let transport_dir = TestTransportDir::new();
        let error = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "../gateway-a",
            &policy_dir.root,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid config publication replica id")
        );
    }

    #[test]
    fn config_publication_transport_compaction_keeps_latest_fully_acked_events() {
        let policy_dir = TestPolicyDir::new();
        policy_dir.write_policy(3);
        let transport_dir = TestTransportDir::new();
        let first_event = complete_event();
        let mut second_event = complete_event();
        second_event.activated_revision_id = PolicyRevisionId::new();
        second_event.previous_active_revision_id = Some(PolicyRevisionId::new());

        let first_publish = publish_config_publication_event_to_gateway_transport(
            &first_event,
            &transport_dir.root,
        )
        .unwrap();
        let second_publish = publish_config_publication_event_to_gateway_transport(
            &second_event,
            &transport_dir.root,
        )
        .unwrap();

        let _ = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-a",
            &policy_dir.root,
        )
        .unwrap();
        let _ = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-b",
            &policy_dir.root,
        )
        .unwrap();

        let compaction = compact_config_publication_transport(&transport_dir.root, 1).unwrap();
        assert_eq!(compaction.replica_count, 2);
        assert_eq!(compaction.eligible_event_count, 2);
        assert_eq!(compaction.removed_event_count, 1);
        assert_eq!(compaction.retained_event_count, 1);
        assert!(!first_publish.event_path.exists());
        assert!(second_publish.event_path.exists());
        assert!(
            !config_publication_transport_ack_dir(&transport_dir.root, "gateway-a")
                .join(format!("{}.ack.json", first_publish.event_id))
                .exists()
        );
        assert!(
            config_publication_transport_ack_dir(&transport_dir.root, "gateway-a")
                .join(format!("{}.ack.json", second_publish.event_id))
                .exists()
        );
    }

    #[test]
    fn config_publication_transport_compaction_skips_events_without_all_replica_acks() {
        let policy_dir = TestPolicyDir::new();
        policy_dir.write_policy(3);
        let transport_dir = TestTransportDir::new();
        let event = complete_event();

        let publish =
            publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root)
                .unwrap();
        let _ = deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_dir.root,
            "gateway-a",
            &policy_dir.root,
        )
        .unwrap();
        std::fs::create_dir_all(config_publication_transport_ack_dir(
            &transport_dir.root,
            "gateway-b",
        ))
        .unwrap();

        let compaction = compact_config_publication_transport(&transport_dir.root, 0).unwrap();
        assert_eq!(compaction.replica_count, 2);
        assert_eq!(compaction.eligible_event_count, 0);
        assert_eq!(compaction.removed_event_count, 0);
        assert_eq!(compaction.retained_event_count, 1);
        assert!(publish.event_path.exists());
    }
}

#[cfg(test)]
#[path = "../tests/src/runtime_policy_last_known_good.rs"]
mod runtime_policy_last_known_good_tests;
