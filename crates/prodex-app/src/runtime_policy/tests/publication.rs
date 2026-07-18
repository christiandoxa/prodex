#![cfg(test)]

use super::{
    ConfigPublicationEventPlan, ConfigPublicationEventTarget, PolicyRevisionId, TenantId,
    clear_runtime_policy_cache, compact_config_publication_transport,
    config_publication_transport_ack_dir, deliver_config_publication_event_to_gateway_runtime,
    deliver_pending_config_publication_events_to_gateway_runtime,
    deliver_pending_config_publication_events_with_activation,
    publish_config_publication_event_to_gateway_transport,
};
use anyhow::bail;
use std::path::PathBuf;

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
    let delivery =
        deliver_config_publication_event_to_gateway_runtime(&complete_event(), &policy_dir.root)
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
        publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root).unwrap();
    let second_publish =
        publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root).unwrap();

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
fn config_publication_ack_waits_for_live_activation() {
    let policy_dir = TestPolicyDir::new();
    policy_dir.write_policy(3);
    let transport_dir = TestTransportDir::new();
    publish_config_publication_event_to_gateway_transport(&complete_event(), &transport_dir.root)
        .unwrap();

    assert!(
        deliver_pending_config_publication_events_with_activation(
            &transport_dir.root,
            "gateway-live",
            &policy_dir.root,
            |_| bail!("live activation failed"),
        )
        .is_err()
    );
    let retried = deliver_pending_config_publication_events_to_gateway_runtime(
        &transport_dir.root,
        "gateway-live",
        &policy_dir.root,
    )
    .unwrap();
    assert_eq!(retried.delivered_event_count, 1);
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

    let first_publish =
        publish_config_publication_event_to_gateway_transport(&first_event, &transport_dir.root)
            .unwrap();
    let second_publish =
        publish_config_publication_event_to_gateway_transport(&second_event, &transport_dir.root)
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
        publish_config_publication_event_to_gateway_transport(&event, &transport_dir.root).unwrap();
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
