use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_observability::{
    ConfigPublicationDeliveryMetricPlan, ConfigPublicationDeliveryResult,
    ConfigPublicationDeliveryTarget, plan_config_publication_delivery_metric,
};

use prodex_runtime_policy::RuntimePolicyCacheInvalidationPlan;
#[cfg(test)]
pub(super) use prodex_runtime_policy::clear_runtime_policy_cache;
pub(super) use prodex_runtime_policy::{
    RuntimeLogFormat, RuntimePolicySummary, ensure_runtime_policy_valid, runtime_policy_proxy,
    runtime_policy_runtime, runtime_policy_secrets, runtime_policy_summary,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RuntimePolicyPublicationDeliveryPlan {
    pub root: PathBuf,
    pub gateway_cache_refreshed: bool,
    pub runtime_policy_invalidation: RuntimePolicyCacheInvalidationPlan,
    pub runtime_policy_version: Option<u32>,
    pub delivery_metrics: [ConfigPublicationDeliveryMetricPlan; 2],
}

pub(super) fn deliver_config_publication_event_to_gateway_runtime(
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

    let runtime_policy_invalidation =
        prodex_runtime_policy::plan_runtime_policy_cache_invalidation(root);
    let runtime_policy = prodex_runtime_policy::load_runtime_policy_cached(root)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_config::ConfigPublicationEventPlan;
    use prodex_domain::{PolicyRevisionId, TenantId};

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
}
