use anyhow::{Context, Result};
use prodex_application::ApplicationInspectionPlan;
use prodex_domain::{
    ClassificationRule, ClassificationRuleSet, ClassificationRuleSetChecksum,
    ClassificationRuleSetRevisionId, CompiledClassificationRuleSet, DataClassification,
    DetectorRevisionId, FindingKind, TenantId, compile_classification_rule_set,
};
use prodex_runtime_policy::RuntimePolicyInspectionPattern;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::runtime_proxy::presidio::local::RuntimeTenantDetectorPatterns;
use crate::runtime_proxy::presidio::{
    RuntimePresidioWebSocketInspection, apply_runtime_presidio_redaction_to_request,
    apply_runtime_presidio_redaction_to_request_with_rules,
    apply_runtime_presidio_redaction_to_websocket_text,
    apply_runtime_presidio_redaction_to_websocket_text_with_rules,
};
use crate::shared_types::RuntimeProxyRequest;

use super::local_rewrite::RuntimeLocalRewriteProxyShared;

const RUNTIME_CLASSIFICATION_RULES_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_RUNTIME_CLASSIFICATION_RULES_ARTIFACT_BYTES: usize = 1024 * 1024;
pub(super) const MAX_RUNTIME_CLASSIFICATION_RULES_TENANTS: usize = 64;
const MAX_RUNTIME_CLASSIFICATION_RULES_PATTERNS: usize = 16;
const MAX_RUNTIME_CLASSIFICATION_PATTERN_ID_BYTES: usize = 64;
const BOOTSTRAP_DETECTOR_REVISION: &str = "runtime-inspection-v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeClassificationRulesArtifact {
    schema_version: u32,
    detector_revision: String,
    patterns: Vec<RuntimeClassificationPatternArtifact>,
    classification_revision: String,
    classification_checksum: String,
    unsupported_coverage_floor: DataClassification,
    classification_rules: Vec<RuntimeClassificationRuleArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeClassificationPatternArtifact {
    id: String,
    pattern: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeClassificationRuleArtifact {
    finding_kind: FindingKind,
    classification: DataClassification,
}

#[derive(Clone)]
pub(super) struct RuntimeClassificationRulesSnapshot {
    detector_revision: DetectorRevisionId,
    patterns: RuntimeTenantDetectorPatterns,
    classification_rules: CompiledClassificationRuleSet,
}

impl std::fmt::Debug for RuntimeClassificationRulesSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeClassificationRulesSnapshot")
            .field("detector_revision", &self.detector_revision)
            .field("patterns", &"<redacted>")
            .field("classification_rules", &self.classification_rules)
            .finish()
    }
}

impl RuntimeClassificationRulesSnapshot {
    pub(super) fn detector_revision(&self) -> &DetectorRevisionId {
        &self.detector_revision
    }

    pub(super) fn patterns(&self) -> &RuntimeTenantDetectorPatterns {
        &self.patterns
    }

    pub(super) fn classification_rules(&self) -> &CompiledClassificationRuleSet {
        &self.classification_rules
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeClassificationRulesSnapshotSet {
    tenant_snapshots: BTreeMap<TenantId, Arc<RuntimeClassificationRulesSnapshot>>,
    fallback: Option<Arc<RuntimeClassificationRulesSnapshot>>,
}

impl RuntimeClassificationRulesSnapshotSet {
    pub(super) fn bootstrap(
        settings: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
        allow_fallback: bool,
    ) -> Result<Self> {
        let detector_revision = DetectorRevisionId::new(BOOTSTRAP_DETECTOR_REVISION)
            .context("invalid bootstrap detector revision")?;
        let classification_rules =
            crate::runtime_governance::build_runtime_governance_snapshot(settings)?
                .classification_rules;
        let mut grouped = BTreeMap::<TenantId, Vec<RuntimePolicyInspectionPattern>>::new();
        for pattern in &settings.inspection_patterns {
            grouped
                .entry(pattern.tenant_id)
                .or_default()
                .push(pattern.clone());
        }
        if grouped.len() > MAX_RUNTIME_CLASSIFICATION_RULES_TENANTS {
            anyhow::bail!("classification rules tenant limit exceeded");
        }
        let tenant_snapshots = grouped
            .into_iter()
            .map(|(tenant_id, patterns)| {
                let patterns = RuntimeTenantDetectorPatterns::compile(&patterns)?;
                Ok((
                    tenant_id,
                    Arc::new(RuntimeClassificationRulesSnapshot {
                        detector_revision: detector_revision.clone(),
                        patterns,
                        classification_rules: classification_rules.clone(),
                    }),
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let fallback = allow_fallback.then(|| {
            Arc::new(RuntimeClassificationRulesSnapshot {
                detector_revision,
                patterns: RuntimeTenantDetectorPatterns::default(),
                classification_rules,
            })
        });
        Ok(Self {
            tenant_snapshots,
            fallback,
        })
    }

    pub(super) fn snapshot_for(
        &self,
        tenant_id: TenantId,
    ) -> Option<Arc<RuntimeClassificationRulesSnapshot>> {
        self.tenant_snapshots
            .get(&tenant_id)
            .cloned()
            .or_else(|| self.fallback.clone())
    }

    pub(super) fn with_tenant_snapshot(
        &self,
        tenant_id: TenantId,
        snapshot: RuntimeClassificationRulesSnapshot,
    ) -> Result<Self> {
        if !self.tenant_snapshots.contains_key(&tenant_id)
            && self.tenant_snapshots.len() >= MAX_RUNTIME_CLASSIFICATION_RULES_TENANTS
        {
            anyhow::bail!("classification rules tenant limit exceeded");
        }
        let mut next = self.clone();
        next.tenant_snapshots.insert(tenant_id, Arc::new(snapshot));
        Ok(next)
    }
}

pub(super) fn compile_runtime_classification_rules_artifact(
    tenant_id: TenantId,
    artifact: &[u8],
) -> Result<RuntimeClassificationRulesSnapshot> {
    if artifact.is_empty() || artifact.len() > MAX_RUNTIME_CLASSIFICATION_RULES_ARTIFACT_BYTES {
        anyhow::bail!("classification rules artifact size is invalid");
    }
    let artifact: RuntimeClassificationRulesArtifact = serde_json::from_slice(artifact)
        .context("classification rules artifact schema is invalid")?;
    if artifact.schema_version != RUNTIME_CLASSIFICATION_RULES_SCHEMA_VERSION {
        anyhow::bail!("classification rules artifact schema version is unsupported");
    }
    if artifact.patterns.len() > MAX_RUNTIME_CLASSIFICATION_RULES_PATTERNS {
        anyhow::bail!("classification rules pattern limit exceeded");
    }
    let detector_revision = DetectorRevisionId::new(artifact.detector_revision)
        .context("classification rules detector revision is invalid")?;
    let expected_checksum = runtime_classification_rules_checksum(
        artifact.unsupported_coverage_floor,
        &artifact.classification_rules,
    )?;
    if artifact.classification_checksum != expected_checksum {
        anyhow::bail!("classification rules checksum does not match canonical payload");
    }
    let classification_rules = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new(artifact.classification_revision)
            .context("classification rules revision is invalid")?,
        checksum: ClassificationRuleSetChecksum::new(artifact.classification_checksum)
            .context("classification rules checksum is invalid")?,
        unsupported_coverage_floor: artifact.unsupported_coverage_floor,
        rules: artifact
            .classification_rules
            .into_iter()
            .map(|rule| ClassificationRule {
                finding_kind: rule.finding_kind,
                classification: rule.classification,
            })
            .collect(),
    })
    .context("classification rule set is invalid")?;
    let patterns = artifact
        .patterns
        .into_iter()
        .map(|pattern| {
            if pattern.id.is_empty()
                || pattern.id.len() > MAX_RUNTIME_CLASSIFICATION_PATTERN_ID_BYTES
                || !pattern
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            {
                anyhow::bail!("classification rule pattern id is invalid");
            }
            Ok(RuntimePolicyInspectionPattern {
                tenant_id,
                id: pattern.id,
                pattern: pattern.pattern,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RuntimeClassificationRulesSnapshot {
        detector_revision,
        patterns: RuntimeTenantDetectorPatterns::compile(&patterns)
            .context("classification rules matcher is invalid")?,
        classification_rules,
    })
}

fn runtime_classification_rules_checksum(
    unsupported_coverage_floor: DataClassification,
    rules: &[RuntimeClassificationRuleArtifact],
) -> Result<String> {
    #[derive(Serialize)]
    struct CanonicalClassificationRules<'a> {
        unsupported_coverage_floor: DataClassification,
        rules: &'a [RuntimeClassificationRuleArtifact],
    }

    let mut rules = rules.to_vec();
    rules.sort_by_key(|rule| (rule.finding_kind, rule.classification));
    let encoded = serde_json::to_vec(&CanonicalClassificationRules {
        unsupported_coverage_floor,
        rules: &rules,
    })
    .context("failed to canonicalize classification rules")?;
    Ok(Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn apply_runtime_gateway_classification_to_request(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
) -> Result<ApplicationInspectionPlan> {
    let Some(tenant_id) = tenant_id else {
        return apply_runtime_presidio_redaction_to_request(
            request_id,
            request,
            &shared.runtime_shared,
            legacy_local_enabled,
            None,
        );
    };
    let governance = shared
        .governance_snapshot
        .load_full()
        .snapshot_for(tenant_id)
        .context("tenant governance policy snapshot is unavailable")?;
    let classification = shared
        .classification_rules
        .load_full()
        .snapshot_for(tenant_id)
        .context("tenant classification rules snapshot is unavailable")?;
    apply_runtime_presidio_redaction_to_request_with_rules(
        request_id,
        request,
        &shared.runtime_shared,
        legacy_local_enabled,
        Some(tenant_id),
        &governance.config,
        classification.patterns(),
        classification.detector_revision(),
    )
}

pub(super) fn apply_runtime_gateway_classification_to_websocket_text<'a>(
    request_id: u64,
    text: &'a str,
    shared: &RuntimeLocalRewriteProxyShared,
    legacy_local_enabled: bool,
    tenant_id: Option<TenantId>,
) -> Result<RuntimePresidioWebSocketInspection<'a>> {
    let Some(tenant_id) = tenant_id else {
        return apply_runtime_presidio_redaction_to_websocket_text(
            request_id,
            text,
            &shared.runtime_shared,
            legacy_local_enabled,
            None,
        );
    };
    let governance = shared
        .governance_snapshot
        .load_full()
        .snapshot_for(tenant_id)
        .context("tenant governance policy snapshot is unavailable")?;
    let classification = shared
        .classification_rules
        .load_full()
        .snapshot_for(tenant_id)
        .context("tenant classification rules snapshot is unavailable")?;
    apply_runtime_presidio_redaction_to_websocket_text_with_rules(
        request_id,
        text,
        &shared.runtime_shared,
        legacy_local_enabled,
        Some(tenant_id),
        &governance.config,
        classification.patterns(),
        classification.detector_revision(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_proxy::presidio::local::runtime_local_inspect_and_mask_for_tenant;
    use prodex_domain::{
        ClassificationRequest, InspectionCoverage, InspectionLimits, InspectionResult,
        classify_inspection,
    };

    fn tenant(value: u128) -> TenantId {
        TenantId::from_uuid(uuid::Uuid::from_u128(value))
    }

    fn artifact(revision: &str, pattern: &str) -> Vec<u8> {
        let classification_rules = vec![RuntimeClassificationRuleArtifact {
            finding_kind: FindingKind::TenantSensitive,
            classification: DataClassification::Confidential,
        }];
        let checksum = runtime_classification_rules_checksum(
            DataClassification::Restricted,
            &classification_rules,
        )
        .unwrap();
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "detector_revision": revision,
            "patterns": [{"id": "customer-secret", "pattern": pattern}],
            "classification_revision": "classification-v1",
            "classification_checksum": checksum,
            "unsupported_coverage_floor": "restricted",
            "classification_rules": [{
                "finding_kind": "tenant_sensitive",
                "classification": "confidential"
            }]
        }))
        .unwrap()
    }

    #[test]
    fn classification_rules_are_tenant_bound_and_unicode_safe() {
        let first = tenant(1);
        let second = tenant(2);
        let first_snapshot = compile_runtime_classification_rules_artifact(
            first,
            &artifact("detectors-a", "顧客-*秘密"),
        )
        .unwrap();
        let second_snapshot = compile_runtime_classification_rules_artifact(
            second,
            &artifact("detectors-b", "other-secret"),
        )
        .unwrap();
        let set = RuntimeClassificationRulesSnapshotSet::bootstrap(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            false,
        )
        .unwrap()
        .with_tenant_snapshot(first, first_snapshot)
        .unwrap()
        .with_tenant_snapshot(second, second_snapshot)
        .unwrap();

        let first_snapshot = set.snapshot_for(first).unwrap();
        let inspected = runtime_local_inspect_and_mask_for_tenant(
            "顧客-東京秘密 other-secret".as_bytes().to_vec(),
            first_snapshot.patterns(),
            Some(first),
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(inspected.body).unwrap(),
            "<redacted> other-secret"
        );
        assert_eq!(first_snapshot.detector_revision().as_str(), "detectors-a");
        let inspection = InspectionResult::new(
            InspectionCoverage::Unsupported,
            DataClassification::Internal,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            DetectorRevisionId::new("detectors-a").unwrap(),
            InspectionLimits::default(),
        )
        .unwrap();
        let classification = classify_inspection(
            first_snapshot.classification_rules(),
            ClassificationRequest {
                inspection: &inspection,
                trusted_label: None,
                untrusted_label: None,
                prior_classification: None,
                session_floor: DataClassification::Public,
                route_floor: DataClassification::Public,
                request_risk_floor: DataClassification::Public,
            },
        )
        .unwrap();
        assert_eq!(
            classification.classification(),
            DataClassification::Restricted
        );
        assert!(set.snapshot_for(tenant(3)).is_none());
    }

    #[test]
    fn invalid_artifact_does_not_evict_last_known_good_snapshot() {
        let tenant_id = tenant(1);
        let set = RuntimeClassificationRulesSnapshotSet::bootstrap(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            false,
        )
        .unwrap()
        .with_tenant_snapshot(
            tenant_id,
            compile_runtime_classification_rules_artifact(
                tenant_id,
                &artifact("detectors-good", "secret-*value"),
            )
            .unwrap(),
        )
        .unwrap();
        let invalid = br#"{"schema_version":1,"detector_revision":"bad revision","patterns":[]}"#;

        assert!(compile_runtime_classification_rules_artifact(tenant_id, invalid).is_err());
        assert_eq!(
            set.snapshot_for(tenant_id)
                .unwrap()
                .detector_revision()
                .as_str(),
            "detectors-good"
        );
    }

    #[test]
    fn committed_swap_changes_subsequent_inspection_revision() {
        let tenant_id = tenant(1);
        let initial = RuntimeClassificationRulesSnapshotSet::bootstrap(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            false,
        )
        .unwrap()
        .with_tenant_snapshot(
            tenant_id,
            compile_runtime_classification_rules_artifact(
                tenant_id,
                &artifact("detectors-v1", "secret-*one"),
            )
            .unwrap(),
        )
        .unwrap();
        let authority = arc_swap::ArcSwap::from_pointee(initial);
        assert_eq!(
            authority
                .load()
                .snapshot_for(tenant_id)
                .unwrap()
                .detector_revision()
                .as_str(),
            "detectors-v1"
        );

        let next = authority
            .load_full()
            .with_tenant_snapshot(
                tenant_id,
                compile_runtime_classification_rules_artifact(
                    tenant_id,
                    &artifact("detectors-v2", "secret-*two"),
                )
                .unwrap(),
            )
            .unwrap();
        authority.store(Arc::new(next));

        assert_eq!(
            authority
                .load()
                .snapshot_for(tenant_id)
                .unwrap()
                .detector_revision()
                .as_str(),
            "detectors-v2"
        );
    }

    #[test]
    fn classification_rules_schema_is_strict_and_bounded() {
        let tenant_id = tenant(1);
        let mut unknown: serde_json::Value =
            serde_json::from_slice(&artifact("v1", "secret-*value")).unwrap();
        unknown["valid"] = serde_json::Value::Bool(true);
        let mut unsupported: serde_json::Value =
            serde_json::from_slice(&artifact("v1", "secret-*value")).unwrap();
        unsupported["schema_version"] = serde_json::Value::from(2);

        assert!(
            compile_runtime_classification_rules_artifact(
                tenant_id,
                &serde_json::to_vec(&unknown).unwrap(),
            )
            .is_err()
        );
        assert!(
            compile_runtime_classification_rules_artifact(
                tenant_id,
                &serde_json::to_vec(&unsupported).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn classification_checksum_rejects_tampered_rule_content() {
        let tenant_id = tenant(1);
        let mut tampered: serde_json::Value =
            serde_json::from_slice(&artifact("v1", "secret-*value")).unwrap();
        tampered["unsupported_coverage_floor"] = serde_json::json!("internal");

        assert!(
            compile_runtime_classification_rules_artifact(
                tenant_id,
                &serde_json::to_vec(&tampered).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn classification_inspection_boundary_is_memory_only() {
        let source = include_str!("local_rewrite_classification_rules.rs");
        let boundary = source
            .split("pub(super) fn apply_runtime_gateway_classification_to_request")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert!(boundary.contains(".load_full()"));
        for forbidden in [
            "GovernanceSqliteRepository",
            "PostgresRepository",
            ".load_snapshot(",
            "governance_load_snapshot",
        ] {
            assert!(!boundary.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn authenticated_http_and_gemini_paths_use_tenant_authority() {
        let http = include_str!("local_rewrite_pipeline_governance.rs");
        let data_plane = include_str!("local_rewrite_application_data_plane.rs");
        let gemini = include_str!("local_rewrite_gemini_live.rs");
        let gemini_session = include_str!("local_rewrite_gemini_live/session.rs");

        assert!(http.contains("apply_runtime_gateway_classification_to_request"));
        assert!(data_plane.contains("classification.classification_rules().clone()"));
        assert!(gemini.contains("apply_runtime_gateway_classification_to_websocket_text"));
        assert_eq!(
            gemini_session
                .matches("apply_runtime_gateway_classification_to_websocket_text")
                .count(),
            2
        );
        for source in [http, gemini, gemini_session] {
            assert!(!source.contains("apply_runtime_presidio_redaction_to_request("));
            assert!(!source.contains("apply_runtime_presidio_redaction_to_websocket_text("));
        }
    }

    #[test]
    fn documented_classification_rules_example_compiles() {
        let snapshot = compile_runtime_classification_rules_artifact(
            tenant(1),
            include_bytes!("../../../../../deploy/governance-classification-rules.example.json"),
        )
        .unwrap();

        assert_eq!(snapshot.detector_revision().as_str(), "detectors-v1");
        assert_eq!(
            snapshot.classification_rules().checksum().as_str(),
            "867cd7a4ceb2c1f0beb5f0662d71c059ce65ff0e2056be7aa7d650e5772cf3d7"
        );
    }
}
