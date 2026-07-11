use super::*;

struct TestPolicyDir {
    root: PathBuf,
}

impl TestPolicyDir {
    fn new() -> Self {
        clear_runtime_policy_cache();
        let root = std::env::temp_dir().join(format!(
            "prodex-config-publication-lkg-{}-{}",
            std::process::id(),
            next_config_publication_timestamp_ms(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write_policy(&self, worker_count: u32) {
        std::fs::write(
            self.root.join("policy.toml"),
            format!("version = 1\n\n[runtime_proxy]\nworker_count = {worker_count}\n"),
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

#[test]
fn failed_config_publication_reload_keeps_last_known_good_and_retries_event() {
    let policy_dir = TestPolicyDir::new();
    policy_dir.write_policy(4);
    let loaded = prodex_runtime_policy::load_runtime_policy_cached(&policy_dir.root)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.runtime_proxy.worker_count, Some(4));

    let transport_root = policy_dir.root.join("transport");
    let event = ConfigPublicationEventPlan {
        tenant_id: TenantId::new(),
        activated_revision_id: PolicyRevisionId::new(),
        previous_active_revision_id: Some(PolicyRevisionId::new()),
        last_known_good_revision_id: None,
        targets: [
            ConfigPublicationEventTarget::GatewayCacheRefresh,
            ConfigPublicationEventTarget::RuntimePolicyReload,
        ],
    };
    publish_config_publication_event_to_gateway_transport(&event, &transport_root).unwrap();

    std::fs::write(policy_dir.root.join("policy.toml"), "version = [invalid").unwrap();
    assert!(
        deliver_pending_config_publication_events_to_gateway_runtime(
            &transport_root,
            "gateway-lkg",
            &policy_dir.root,
        )
        .is_err()
    );
    let last_known_good = prodex_runtime_policy::load_runtime_policy_cached(&policy_dir.root)
        .unwrap()
        .unwrap();
    assert_eq!(last_known_good.runtime_proxy.worker_count, Some(4));

    policy_dir.write_policy(6);
    let retried = deliver_pending_config_publication_events_to_gateway_runtime(
        &transport_root,
        "gateway-lkg",
        &policy_dir.root,
    )
    .unwrap();
    assert_eq!(retried.delivered_event_count, 1);
    assert_eq!(retried.delivery_metrics[0].increment, 1);
    assert_eq!(retried.delivery_metrics[1].increment, 1);

    let repeated = deliver_pending_config_publication_events_to_gateway_runtime(
        &transport_root,
        "gateway-lkg",
        &policy_dir.root,
    )
    .unwrap();
    assert_eq!(repeated.delivered_event_count, 0);
    assert_eq!(repeated.delivery_metrics[0].increment, 0);
    assert_eq!(repeated.delivery_metrics[1].increment, 0);
}
