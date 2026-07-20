use anyhow::{Context, Result};
use prodex_application::ApplicationGovernanceSnapshot;
use prodex_domain::{
    AuditDetailLevel, CanonicalRoute, Channel, ClassificationRule, ClassificationRuleSet,
    ClassificationRuleSetChecksum, ClassificationRuleSetRevisionId, DataClassification,
    DataModality, GovernanceObligation, GovernancePolicyArtifact, GovernancePolicyRule,
    GovernancePolicyRuleId, GovernedAction, InspectionCoverage, NetworkZone, PolicyEffect,
    PolicyReasonCode, PolicyRuleCondition, PolicySelector, ProviderTrustTier, RequestRisk,
    compile_classification_rule_set, compile_governance_policy,
};
use prodex_runtime_policy::{RuntimeGovernanceMode, RuntimePolicyGovernanceSettings};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

mod runtime_governance_selectors;
use runtime_governance_selectors::runtime_governance_principal_selectors;

pub(crate) const MAX_RUNTIME_GOVERNANCE_ARTIFACT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS: usize = 64;

#[derive(Clone)]
pub(crate) struct RuntimeGovernanceAuthoritySnapshot {
    pub(crate) application: ApplicationGovernanceSnapshot,
    pub(crate) config: prodex_config::GovernanceConfig,
    pub(crate) routing_score_revision: u64,
}

impl std::fmt::Debug for RuntimeGovernanceAuthoritySnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeGovernanceAuthoritySnapshot")
            .field("application", &self.application)
            .field("mode", &self.config.mode)
            .field("routing_score_revision", &self.routing_score_revision)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGovernanceAuthoritySnapshotSet {
    tenant_snapshots: BTreeMap<prodex_domain::TenantId, Arc<RuntimeGovernanceAuthoritySnapshot>>,
    fallback: Option<Arc<RuntimeGovernanceAuthoritySnapshot>>,
}

impl RuntimeGovernanceAuthoritySnapshotSet {
    pub(crate) fn bootstrap(
        snapshot: RuntimeGovernanceAuthoritySnapshot,
        allow_fallback: bool,
    ) -> Self {
        Self {
            tenant_snapshots: BTreeMap::new(),
            fallback: allow_fallback.then(|| Arc::new(snapshot)),
        }
    }

    pub(crate) fn snapshot_for(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Option<Arc<RuntimeGovernanceAuthoritySnapshot>> {
        self.tenant_snapshots
            .get(&tenant_id)
            .cloned()
            .or_else(|| self.fallback.clone())
    }

    pub(crate) fn with_tenant_snapshot(
        &self,
        tenant_id: prodex_domain::TenantId,
        snapshot: RuntimeGovernanceAuthoritySnapshot,
    ) -> Result<Self> {
        if !self.tenant_snapshots.contains_key(&tenant_id)
            && self.tenant_snapshots.len() >= MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS
        {
            anyhow::bail!("governance authority tenant limit exceeded");
        }
        let mut next = self.clone();
        next.tenant_snapshots.insert(tenant_id, Arc::new(snapshot));
        Ok(next)
    }
}

pub(crate) fn compile_runtime_governance_artifact(
    artifact: &[u8],
) -> Result<RuntimeGovernanceAuthoritySnapshot> {
    if artifact.is_empty() || artifact.len() > MAX_RUNTIME_GOVERNANCE_ARTIFACT_BYTES {
        anyhow::bail!("governance artifact size is invalid");
    }
    let settings: RuntimePolicyGovernanceSettings =
        serde_json::from_slice(artifact).context("governance artifact schema is invalid")?;
    compile_runtime_governance_settings(&settings)
}

pub(crate) fn compile_runtime_governance_artifact_for_deployment(
    artifact: &[u8],
    deployment_mode: prodex_config::GovernanceMode,
) -> Result<RuntimeGovernanceAuthoritySnapshot> {
    let snapshot = compile_runtime_governance_artifact(artifact)?;
    let allowed = match deployment_mode {
        prodex_config::GovernanceMode::Personal => true,
        prodex_config::GovernanceMode::EnterpriseObserve => {
            snapshot.config.mode != prodex_config::GovernanceMode::Personal
        }
        prodex_config::GovernanceMode::EnterpriseEnforce => matches!(
            snapshot.config.mode,
            prodex_config::GovernanceMode::EnterpriseEnforce
                | prodex_config::GovernanceMode::BankEnforce
        ),
        prodex_config::GovernanceMode::BankEnforce => {
            snapshot.config.mode == prodex_config::GovernanceMode::BankEnforce
        }
    };
    if !allowed {
        anyhow::bail!("governance artifact mode is below the deployment enforcement floor");
    }
    Ok(snapshot)
}

pub(crate) fn compile_runtime_governance_settings(
    settings: &RuntimePolicyGovernanceSettings,
) -> Result<RuntimeGovernanceAuthoritySnapshot> {
    prodex_runtime_policy::validate_runtime_governance_settings(
        settings,
        Path::new("<governance-artifact>"),
    )
    .context("governance artifact policy is invalid")?;
    Ok(RuntimeGovernanceAuthoritySnapshot {
        application: build_runtime_governance_snapshot(settings)?,
        config: runtime_governance_config(settings),
        routing_score_revision: settings.routing_score_revision.unwrap_or(1),
    })
}

pub(crate) fn runtime_governance_config(
    settings: &RuntimePolicyGovernanceSettings,
) -> prodex_config::GovernanceConfig {
    use prodex_config::{
        GovernanceConfig, GovernanceMode, GovernancePolicyFailureMode, GovernanceRolloutMode,
        GovernanceSessionConfig, GovernanceUnknownClassificationBehavior,
    };
    use prodex_runtime_policy::RuntimeGovernanceRolloutMode;

    let mode = match settings.mode {
        RuntimeGovernanceMode::Personal => GovernanceMode::Personal,
        RuntimeGovernanceMode::EnterpriseObserve => GovernanceMode::EnterpriseObserve,
        RuntimeGovernanceMode::EnterpriseEnforce => GovernanceMode::EnterpriseEnforce,
        RuntimeGovernanceMode::BankEnforce => GovernanceMode::BankEnforce,
    };
    let rollout = |value| match value {
        RuntimeGovernanceRolloutMode::Off => GovernanceRolloutMode::Off,
        RuntimeGovernanceRolloutMode::Observe => GovernanceRolloutMode::Observe,
        RuntimeGovernanceRolloutMode::Enforce => GovernanceRolloutMode::Enforce,
    };
    GovernanceConfig {
        config_version: settings.config_version,
        mode,
        inspection: rollout(settings.inspection),
        classification: rollout(settings.classification),
        policy: rollout(settings.policy),
        routing: rollout(settings.routing),
        mandatory_audit: settings.mandatory_audit,
        anonymous_data_plane: settings.anonymous_data_plane,
        raw_secret_sources: settings.raw_secret_sources,
        classification_default: runtime_governance_classification(settings.classification_default),
        classification_unknown: match settings.classification_unknown {
            prodex_runtime_policy::RuntimeGovernanceUnknownClassificationBehavior::UseDefault => {
                GovernanceUnknownClassificationBehavior::UseDefault
            }
            prodex_runtime_policy::RuntimeGovernanceUnknownClassificationBehavior::Deny => {
                GovernanceUnknownClassificationBehavior::Deny
            }
        },
        policy_failure_mode: match settings.policy_failure_mode {
            prodex_runtime_policy::RuntimeGovernancePolicyFailureMode::Open => {
                GovernancePolicyFailureMode::Open
            }
            prodex_runtime_policy::RuntimeGovernancePolicyFailureMode::Closed => {
                GovernancePolicyFailureMode::Closed
            }
        },
        active_policy_revision: settings.active_policy_revision,
        session: GovernanceSessionConfig {
            absolute_timeout_seconds: settings.session.absolute_timeout_seconds,
            idle_timeout_seconds: settings.session.idle_timeout_seconds,
            max_concurrent: settings.session.max_concurrent,
        },
    }
}

const fn runtime_governance_classification(
    value: prodex_runtime_policy::RuntimeGovernanceDataClassification,
) -> DataClassification {
    match value {
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Public => {
            DataClassification::Public
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal => {
            DataClassification::Internal
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Confidential => {
            DataClassification::Confidential
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted => {
            DataClassification::Restricted
        }
    }
}

pub(crate) fn build_runtime_governance_snapshot(
    settings: &RuntimePolicyGovernanceSettings,
) -> Result<ApplicationGovernanceSnapshot> {
    let classification_rules = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new(
            settings
                .classification_revision
                .as_deref()
                .unwrap_or("builtin-classification-v1"),
        )
        .context("invalid governance classification revision")?,
        checksum: ClassificationRuleSetChecksum::new(
            settings
                .classification_checksum
                .as_deref()
                .unwrap_or("builtin-classification-v1"),
        )
        .context("invalid governance classification checksum")?,
        unsupported_coverage_floor: match settings.classification_unknown {
            prodex_runtime_policy::RuntimeGovernanceUnknownClassificationBehavior::UseDefault => {
                runtime_governance_classification(settings.classification_default)
            }
            prodex_runtime_policy::RuntimeGovernanceUnknownClassificationBehavior::Deny => {
                DataClassification::Restricted
            }
        },
        rules: prodex_domain::FindingKind::ALL
            .into_iter()
            .map(|finding_kind| ClassificationRule {
                finding_kind,
                classification: finding_kind.minimum_classification(),
            })
            .collect(),
    })
    .context("invalid governance classification snapshot")?;
    let policy = compile_governance_policy(GovernancePolicyArtifact {
        revision: settings.policy_revision.unwrap_or_default(),
        valid_until_unix_ms: settings.policy_valid_until_unix_ms.unwrap_or(u64::MAX),
        default_effect: match settings.policy_failure_mode {
            prodex_runtime_policy::RuntimeGovernancePolicyFailureMode::Open => PolicyEffect::Allow,
            prodex_runtime_policy::RuntimeGovernancePolicyFailureMode::Closed => PolicyEffect::Deny,
        },
        rules: runtime_governance_rules(settings)?,
    })
    .context("invalid governance policy snapshot")?;
    Ok(ApplicationGovernanceSnapshot {
        classification_rules,
        policy,
    })
}

fn runtime_governance_rules(
    settings: &RuntimePolicyGovernanceSettings,
) -> Result<Vec<GovernancePolicyRule>> {
    let mut rules = runtime_builtin_governance_rules(settings.mode)?;
    rules.extend(
        settings
            .policy_rules
            .iter()
            .map(runtime_governance_policy_rule)
            .collect::<Result<Vec<_>>>()?,
    );
    Ok(rules)
}

fn runtime_builtin_governance_rules(
    mode: RuntimeGovernanceMode,
) -> Result<Vec<GovernancePolicyRule>> {
    if mode == RuntimeGovernanceMode::Personal {
        return Ok(Vec::new());
    }
    let mut rules = vec![runtime_governance_rule(
        "builtin.coverage-unsupported",
        PolicyRuleCondition {
            inspection_coverage: Some(InspectionCoverage::Unsupported),
            ..runtime_governance_condition()
        },
        PolicyEffect::Deny,
        Vec::new(),
        "policy.inspection_unsupported",
    )?];
    if mode == RuntimeGovernanceMode::BankEnforce {
        rules.push(runtime_governance_rule(
            "builtin.bank-approved-provider",
            runtime_governance_condition(),
            PolicyEffect::Allow,
            vec![
                GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::Enterprise),
                GovernanceObligation::ProhibitRetention,
                GovernanceObligation::ProhibitTrainingUse,
                GovernanceObligation::RequireResponseInspection,
                GovernanceObligation::AuditDetail(AuditDetailLevel::Elevated),
                GovernanceObligation::DenyFallbackOutsideEligibility,
            ],
            "policy.bank_baseline",
        )?);
    }
    rules.push(runtime_governance_rule(
        "builtin.confidential-controls",
        PolicyRuleCondition {
            minimum_classification: Some(DataClassification::Confidential),
            ..runtime_governance_condition()
        },
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::Enterprise),
            GovernanceObligation::ProhibitRetention,
            GovernanceObligation::ProhibitTrainingUse,
            GovernanceObligation::RequireResponseInspection,
            GovernanceObligation::DenyFallbackOutsideEligibility,
        ],
        "policy.confidential_controls",
    )?);
    rules.push(runtime_governance_rule(
        "builtin.restricted-controls",
        PolicyRuleCondition {
            minimum_classification: Some(DataClassification::Restricted),
            ..runtime_governance_condition()
        },
        PolicyEffect::Allow,
        vec![
            GovernanceObligation::MinimumProviderTrust(ProviderTrustTier::RestrictedApproved),
            GovernanceObligation::RequireLocalExecution,
            GovernanceObligation::RequireMfa,
            GovernanceObligation::RequireReauthentication,
            GovernanceObligation::AuditDetail(AuditDetailLevel::Elevated),
        ],
        "policy.restricted_controls",
    )?);
    Ok(rules)
}

fn runtime_governance_policy_rule(
    rule: &prodex_runtime_policy::RuntimeGovernancePolicyRule,
) -> Result<GovernancePolicyRule> {
    use prodex_runtime_policy::{
        RuntimeGovernancePolicyAction as Action, RuntimeGovernancePolicyChannel as ChannelDto,
        RuntimeGovernancePolicyEffect as Effect, RuntimeGovernancePolicyNetworkZone as Zone,
        RuntimeGovernancePolicyRequestRisk as Risk,
    };

    if rule
        .condition
        .channel
        .is_some_and(|channel| channel != ChannelDto::Api)
    {
        anyhow::bail!("gateway governance policy channel selector must be api");
    }
    let selectors = runtime_governance_principal_selectors(&rule.condition)?;

    Ok(GovernancePolicyRule {
        id: GovernancePolicyRuleId::new(&rule.id).context("invalid governance policy rule id")?,
        condition: PolicyRuleCondition {
            channel: rule.condition.channel.map(|value| match value {
                ChannelDto::Cli => Channel::Cli,
                ChannelDto::Ide => Channel::Ide,
                ChannelDto::Api => Channel::Api,
                ChannelDto::Mcp => Channel::Mcp,
                ChannelDto::InternalService => Channel::InternalService,
            }),
            principal_kind: rule.condition.principal_kind,
            team_id: selectors.team_id,
            project_id: selectors.project_id,
            user_id: selectors.user_id,
            group_id: selectors.group_id,
            department_id: selectors.department_id,
            minimum_role: rule.condition.minimum_role,
            credential_scope: rule.condition.credential_scope,
            action: rule.condition.action.map(|value| match value {
                Action::InvokeModel => GovernedAction::InvokeModel,
                Action::UseTool => GovernedAction::UseTool,
                Action::UploadContent => GovernedAction::UploadContent,
                Action::CompactContext => GovernedAction::CompactContext,
                Action::MutateControlPlane => GovernedAction::MutateControlPlane,
            }),
            route: rule
                .condition
                .route
                .as_deref()
                .map(CanonicalRoute::new)
                .transpose()
                .context("invalid governance policy route")?,
            minimum_classification: rule
                .condition
                .minimum_classification
                .map(runtime_governance_classification),
            inspection_coverage: rule.condition.inspection_coverage,
            minimum_request_risk: rule
                .condition
                .minimum_request_risk
                .map(|value| match value {
                    Risk::Low => RequestRisk::Low,
                    Risk::Elevated => RequestRisk::Elevated,
                    Risk::High => RequestRisk::High,
                    Risk::Critical => RequestRisk::Critical,
                }),
            network_zone: rule.condition.network_zone.map(|value| match value {
                Zone::Local => NetworkZone::Local,
                Zone::TrustedInternal => NetworkZone::TrustedInternal,
                Zone::Partner => NetworkZone::Partner,
                Zone::Public => NetworkZone::Public,
                Zone::Unknown => NetworkZone::Unknown,
            }),
            maximum_session_age_seconds: rule.condition.maximum_session_age_seconds,
            maximum_session_idle_seconds: rule.condition.maximum_session_idle_seconds,
            session_revoked: rule.condition.session_revoked,
            session_mfa_satisfied: rule.condition.session_mfa_satisfied,
            minimum_session_retained_classification: rule
                .condition
                .minimum_session_retained_classification
                .map(runtime_governance_classification),
            minimum_authentication_strength: rule.condition.minimum_authentication_strength,
            environment_mfa_satisfied: rule.condition.environment_mfa_satisfied,
            requested_capability: rule.condition.requested_capability,
            requested_model: rule
                .condition
                .requested_model
                .as_deref()
                .map(PolicySelector::new)
                .transpose()
                .context("invalid governance policy model selector")?,
            requested_tool: rule
                .condition
                .requested_tool
                .as_deref()
                .map(PolicySelector::new)
                .transpose()
                .context("invalid governance policy tool selector")?,
            requested_modality: rule.condition.requested_modality.map(|value| match value {
                prodex_runtime_policy::RuntimeGovernancePolicyDataModality::Text => {
                    DataModality::Text
                }
                prodex_runtime_policy::RuntimeGovernancePolicyDataModality::Image => {
                    DataModality::Image
                }
                prodex_runtime_policy::RuntimeGovernancePolicyDataModality::Audio => {
                    DataModality::Audio
                }
                prodex_runtime_policy::RuntimeGovernancePolicyDataModality::Video => {
                    DataModality::Video
                }
                prodex_runtime_policy::RuntimeGovernancePolicyDataModality::File => {
                    DataModality::File
                }
            }),
            break_glass_required: rule.condition.break_glass_required,
            break_glass_scope: rule
                .condition
                .break_glass_scope
                .as_deref()
                .map(PolicySelector::new)
                .transpose()
                .context("invalid governance policy break-glass selector")?,
            quota_has_headroom: rule.condition.quota_has_headroom,
            quota_reservation_required: rule.condition.quota_reservation_required,
        },
        effect: match rule.effect {
            Effect::Allow => PolicyEffect::Allow,
            Effect::RequireApproval => PolicyEffect::RequireApproval,
            Effect::Deny => PolicyEffect::Deny,
        },
        obligations: rule
            .obligations
            .iter()
            .map(runtime_governance_policy_obligation)
            .collect::<Result<Vec<_>>>()?,
        reason_code: PolicyReasonCode::new(&rule.reason_code)
            .context("invalid governance policy reason code")?,
    })
}

fn runtime_governance_policy_obligation(
    obligation: &prodex_runtime_policy::RuntimeGovernancePolicyObligation,
) -> Result<GovernanceObligation> {
    use prodex_runtime_policy::{
        RuntimeGovernancePolicyAuditDetailLevel as Detail,
        RuntimeGovernancePolicyDataModality as Modality,
        RuntimeGovernancePolicyObligation as Obligation,
        RuntimeGovernanceProviderTrustTier as Trust,
    };

    let selector = |value: &str| {
        PolicySelector::new(value).context("invalid governance policy obligation selector")
    };
    Ok(match obligation {
        Obligation::MaskFinding { finding_kind } => {
            GovernanceObligation::MaskFinding(*finding_kind)
        }
        Obligation::MinimumProviderTrust { trust_tier } => {
            GovernanceObligation::MinimumProviderTrust(match trust_tier {
                Trust::Standard => ProviderTrustTier::Standard,
                Trust::Enterprise => ProviderTrustTier::Enterprise,
                Trust::RestrictedApproved => ProviderTrustTier::RestrictedApproved,
            })
        }
        Obligation::AllowProvider { selector: value } => {
            GovernanceObligation::AllowProvider(selector(value)?)
        }
        Obligation::DenyProvider { selector: value } => {
            GovernanceObligation::DenyProvider(selector(value)?)
        }
        Obligation::RequireLocalExecution => GovernanceObligation::RequireLocalExecution,
        Obligation::ProhibitRetention => GovernanceObligation::ProhibitRetention,
        Obligation::ProhibitTrainingUse => GovernanceObligation::ProhibitTrainingUse,
        Obligation::RequireRegion { selector: value } => {
            GovernanceObligation::RequireRegion(selector(value)?)
        }
        Obligation::DisableTools => GovernanceObligation::DisableTools,
        Obligation::AllowTool { selector: value } => {
            GovernanceObligation::AllowTool(selector(value)?)
        }
        Obligation::AllowModel { selector: value } => {
            GovernanceObligation::AllowModel(selector(value)?)
        }
        Obligation::AllowModality { modality } => {
            GovernanceObligation::AllowModality(match modality {
                Modality::Text => DataModality::Text,
                Modality::Image => DataModality::Image,
                Modality::Audio => DataModality::Audio,
                Modality::Video => DataModality::Video,
                Modality::File => DataModality::File,
            })
        }
        Obligation::MaxInputTokens { value } => GovernanceObligation::MaxInputTokens(*value),
        Obligation::MaxOutputTokens { value } => GovernanceObligation::MaxOutputTokens(*value),
        Obligation::MaxContextTokens { value } => GovernanceObligation::MaxContextTokens(*value),
        Obligation::RequireResponseInspection => GovernanceObligation::RequireResponseInspection,
        Obligation::SessionIdleTimeoutSeconds { value } => {
            GovernanceObligation::SessionIdleTimeoutSeconds(*value)
        }
        Obligation::SessionAbsoluteTimeoutSeconds { value } => {
            GovernanceObligation::SessionAbsoluteTimeoutSeconds(*value)
        }
        Obligation::RequireReauthentication => GovernanceObligation::RequireReauthentication,
        Obligation::RequireMfa => GovernanceObligation::RequireMfa,
        Obligation::AuditDetail { level } => GovernanceObligation::AuditDetail(match level {
            Detail::Minimal => AuditDetailLevel::Minimal,
            Detail::Standard => AuditDetailLevel::Standard,
            Detail::Elevated => AuditDetailLevel::Elevated,
        }),
        Obligation::RequireHumanApproval => GovernanceObligation::RequireHumanApproval,
        Obligation::RetentionSeconds { value } => GovernanceObligation::RetentionSeconds(*value),
        Obligation::DenyFallbackOutsideEligibility => {
            GovernanceObligation::DenyFallbackOutsideEligibility
        }
    })
}

fn runtime_governance_rule(
    id: &str,
    condition: PolicyRuleCondition,
    effect: PolicyEffect,
    obligations: Vec<GovernanceObligation>,
    reason: &str,
) -> Result<GovernancePolicyRule> {
    Ok(GovernancePolicyRule {
        id: GovernancePolicyRuleId::new(id).context("invalid governance rule id")?,
        condition,
        effect,
        obligations,
        reason_code: PolicyReasonCode::new(reason)
            .context("invalid governance policy reason code")?,
    })
}

fn runtime_governance_condition() -> PolicyRuleCondition {
    PolicyRuleCondition::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_application::{
        ApplicationGovernanceRequest, ApplicationInspectionPlan, plan_application_governance,
    };
    use prodex_domain::{
        CanonicalRoute, CapabilitySet, Channel, CredentialScope, DetectorRevisionId,
        EnvironmentContext, GovernedAction, InspectionCoverage, InspectionLimits, InspectionResult,
        NetworkZone, PolicyInput, Principal, PrincipalId, PrincipalKind, PrincipalPolicyAttributes,
        QuotaContext, RequestPolicyAttributes, RequestRisk, Role, SessionPolicyContext,
        TenantContext, TenantId, evaluate_governance_policy,
    };

    #[test]
    fn bank_snapshot_denies_unsupported_inspection() {
        let settings = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::BankEnforce,
            ..RuntimePolicyGovernanceSettings::default()
        };
        let snapshot = build_runtime_governance_snapshot(&settings).unwrap();
        assert!(format!("{snapshot:?}").contains("ApplicationGovernanceSnapshot"));
    }

    #[test]
    fn bank_custom_rules_cannot_remove_builtin_obligations() {
        let tenant_id = TenantId::new();
        let mut settings = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::BankEnforce,
            ..RuntimePolicyGovernanceSettings::default()
        };
        settings.policy_rules = vec![prodex_runtime_policy::RuntimeGovernancePolicyRule {
            id: "custom.allow-api".to_string(),
            condition: prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition {
                channel: Some(prodex_runtime_policy::RuntimeGovernancePolicyChannel::Api),
                ..Default::default()
            },
            effect: prodex_runtime_policy::RuntimeGovernancePolicyEffect::Allow,
            obligations: Vec::new(),
            reason_code: "policy.custom_allow".to_string(),
        }];
        let snapshot = build_runtime_governance_snapshot(&settings).unwrap();
        let principal = Principal::new(
            PrincipalId::new(),
            Some(tenant_id),
            PrincipalKind::ServiceAccount,
            Role::Operator,
            CredentialScope::DataPlane,
        );
        let route = CanonicalRoute::new("responses").unwrap();
        let capabilities = CapabilitySet::new(Vec::new());
        let principal_attributes = PrincipalPolicyAttributes::default();
        let request_attributes = RequestPolicyAttributes::default();
        let decision = evaluate_governance_policy(
            &snapshot.policy,
            &PolicyInput {
                tenant: TenantContext { tenant_id },
                principal: &principal,
                principal_attributes: &principal_attributes,
                channel: Channel::Api,
                credential_scope: CredentialScope::DataPlane,
                session: SessionPolicyContext {
                    age_seconds: 0,
                    idle_seconds: 0,
                    revoked: false,
                    mfa_satisfied: true,
                    retained_classification: DataClassification::Internal,
                },
                action: GovernedAction::InvokeModel,
                route: &route,
                data: prodex_domain::DataPolicyContext {
                    classification: DataClassification::Internal,
                    inspection_coverage: InspectionCoverage::Full,
                },
                request_risk: RequestRisk::Low,
                requested_capabilities: &capabilities,
                request_attributes: &request_attributes,
                quota: QuotaContext {
                    has_headroom: true,
                    reservation_required: true,
                },
                environment: EnvironmentContext {
                    network_zone: NetworkZone::TrustedInternal,
                    authentication_strength: 3,
                    mfa_satisfied: true,
                },
            },
        )
        .unwrap();

        assert_eq!(decision.effect, PolicyEffect::Allow);
        assert!(
            decision
                .obligations
                .contains(&GovernanceObligation::ProhibitRetention)
        );
        assert!(
            decision
                .obligations
                .contains(&GovernanceObligation::RequireResponseInspection)
        );
        assert!(
            decision
                .obligations
                .contains(&GovernanceObligation::DenyFallbackOutsideEligibility)
        );
    }

    fn policy_effect(
        snapshot: &RuntimeGovernanceAuthoritySnapshot,
        tenant_id: TenantId,
    ) -> PolicyEffect {
        let principal = Principal::new(
            PrincipalId::new(),
            Some(tenant_id),
            PrincipalKind::ServiceAccount,
            Role::Operator,
            CredentialScope::DataPlane,
        );
        let inspection = ApplicationInspectionPlan {
            result: InspectionResult::new(
                InspectionCoverage::Unsupported,
                DataClassification::Internal,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                DetectorRevisionId::new("detector-v1").unwrap(),
                InspectionLimits::default(),
            )
            .unwrap(),
            masked_findings: Vec::new(),
        };
        let route = CanonicalRoute::new("responses").unwrap();
        let capabilities = CapabilitySet::new(Vec::new());
        let principal_attributes = PrincipalPolicyAttributes::default();
        let request_attributes = RequestPolicyAttributes::default();
        plan_application_governance(
            &snapshot.application,
            ApplicationGovernanceRequest {
                inspection: &inspection,
                trusted_label: None,
                untrusted_label: None,
                prior_classification: None,
                session_floor: DataClassification::Public,
                route_floor: DataClassification::Public,
                request_risk_floor: DataClassification::Public,
                tenant: TenantContext { tenant_id },
                principal: &principal,
                principal_attributes: &principal_attributes,
                channel: Channel::Api,
                credential_scope: CredentialScope::DataPlane,
                session: SessionPolicyContext {
                    age_seconds: 0,
                    idle_seconds: 0,
                    revoked: false,
                    mfa_satisfied: false,
                    retained_classification: DataClassification::Public,
                },
                action: GovernedAction::InvokeModel,
                route: &route,
                request_risk: RequestRisk::Low,
                requested_capabilities: &capabilities,
                request_attributes: &request_attributes,
                quota: QuotaContext {
                    has_headroom: true,
                    reservation_required: true,
                },
                environment: EnvironmentContext {
                    network_zone: NetworkZone::Unknown,
                    authentication_strength: 1,
                    mfa_satisfied: false,
                },
            },
        )
        .unwrap()
        .policy
        .effect
    }

    #[test]
    fn committed_activation_swap_changes_subsequent_pdp_without_cross_tenant_fallback() {
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();
        let personal = compile_runtime_governance_settings(&RuntimePolicyGovernanceSettings {
            routing_score_revision: Some(11),
            ..RuntimePolicyGovernanceSettings::default()
        })
        .unwrap();
        let mut enterprise_settings = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::EnterpriseObserve,
            routing_score_revision: Some(22),
            ..RuntimePolicyGovernanceSettings::default()
        };
        enterprise_settings.policy_failure_mode =
            prodex_runtime_policy::RuntimeGovernancePolicyFailureMode::Closed;
        let enterprise = compile_runtime_governance_settings(&enterprise_settings).unwrap();
        let set = RuntimeGovernanceAuthoritySnapshotSet::bootstrap(personal.clone(), false)
            .with_tenant_snapshot(tenant_a, personal)
            .unwrap()
            .with_tenant_snapshot(tenant_b, enterprise.clone())
            .unwrap();
        let authority = arc_swap::ArcSwap::from_pointee(set);
        assert_eq!(
            policy_effect(&authority.load().snapshot_for(tenant_a).unwrap(), tenant_a),
            PolicyEffect::Allow
        );
        assert_eq!(
            policy_effect(&authority.load().snapshot_for(tenant_b).unwrap(), tenant_b),
            PolicyEffect::Deny
        );
        assert!(authority.load().snapshot_for(TenantId::new()).is_none());

        let next = authority
            .load_full()
            .with_tenant_snapshot(tenant_a, enterprise)
            .unwrap();
        authority.store(Arc::new(next));
        assert_eq!(
            policy_effect(&authority.load().snapshot_for(tenant_a).unwrap(), tenant_a),
            PolicyEffect::Deny
        );
    }

    #[test]
    fn serialized_policy_rules_drive_tenant_pdp() {
        let tenant_id = TenantId::new();
        let mut settings = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::EnterpriseObserve,
            ..RuntimePolicyGovernanceSettings::default()
        };
        settings.policy_rules = vec![prodex_runtime_policy::RuntimeGovernancePolicyRule {
            id: "deny-api".to_string(),
            condition: prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition {
                channel: Some(prodex_runtime_policy::RuntimeGovernancePolicyChannel::Api),
                credential_scope: Some(CredentialScope::DataPlane),
                ..Default::default()
            },
            effect: prodex_runtime_policy::RuntimeGovernancePolicyEffect::Deny,
            obligations: Vec::new(),
            reason_code: "policy.configured_deny".to_string(),
        }];

        let artifact = serde_json::to_vec(&settings).unwrap();
        let snapshot = compile_runtime_governance_artifact(&artifact).unwrap();

        assert_eq!(policy_effect(&snapshot, tenant_id), PolicyEffect::Deny);
    }

    #[test]
    fn serialized_policy_artifact_rejects_non_api_channel_selectors() {
        for channel in [
            prodex_runtime_policy::RuntimeGovernancePolicyChannel::Cli,
            prodex_runtime_policy::RuntimeGovernancePolicyChannel::Ide,
            prodex_runtime_policy::RuntimeGovernancePolicyChannel::Mcp,
            prodex_runtime_policy::RuntimeGovernancePolicyChannel::InternalService,
        ] {
            let settings = RuntimePolicyGovernanceSettings {
                policy_rules: vec![prodex_runtime_policy::RuntimeGovernancePolicyRule {
                    id: "deny-non-api".to_string(),
                    condition: prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition {
                        channel: Some(channel),
                        ..Default::default()
                    },
                    effect: prodex_runtime_policy::RuntimeGovernancePolicyEffect::Deny,
                    obligations: Vec::new(),
                    reason_code: "policy.non_api".to_string(),
                }],
                ..Default::default()
            };

            assert!(
                compile_runtime_governance_artifact(&serde_json::to_vec(&settings).unwrap())
                    .is_err()
            );
        }
    }

    #[test]
    fn deployment_mode_floor_rejects_policy_downgrade() {
        let artifact = serde_json::to_vec(&RuntimePolicyGovernanceSettings::default()).unwrap();

        assert!(
            compile_runtime_governance_artifact_for_deployment(
                &artifact,
                prodex_config::GovernanceMode::BankEnforce,
            )
            .is_err()
        );
        assert!(
            compile_runtime_governance_artifact_for_deployment(
                &artifact,
                prodex_config::GovernanceMode::EnterpriseEnforce,
            )
            .is_err()
        );
        assert!(
            compile_runtime_governance_artifact_for_deployment(
                &artifact,
                prodex_config::GovernanceMode::Personal,
            )
            .is_ok()
        );
    }

    #[test]
    fn deployment_compiler_rejects_conflicting_policy_obligations() {
        let settings = RuntimePolicyGovernanceSettings {
            policy_rules: vec![prodex_runtime_policy::RuntimeGovernancePolicyRule {
                id: "conflicting-provider-policy".to_string(),
                condition: prodex_runtime_policy::RuntimeGovernancePolicyRuleCondition::default(),
                effect: prodex_runtime_policy::RuntimeGovernancePolicyEffect::Allow,
                obligations: vec![
                    prodex_runtime_policy::RuntimeGovernancePolicyObligation::AllowProvider {
                        selector: "openai".to_string(),
                    },
                    prodex_runtime_policy::RuntimeGovernancePolicyObligation::DenyProvider {
                        selector: "openai".to_string(),
                    },
                ],
                reason_code: "policy.conflicting_provider".to_string(),
            }],
            ..Default::default()
        };
        let artifact = serde_json::to_vec(&settings).unwrap();

        assert!(
            compile_runtime_governance_artifact_for_deployment(
                &artifact,
                prodex_config::GovernanceMode::Personal,
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_artifact_never_evicts_last_valid_tenant_snapshot() {
        let tenant_id = TenantId::new();
        let valid_settings = RuntimePolicyGovernanceSettings::default();
        let valid = compile_runtime_governance_settings(&valid_settings).unwrap();
        let snapshots = arc_swap::ArcSwap::from_pointee(
            RuntimeGovernanceAuthoritySnapshotSet::bootstrap(valid.clone(), false)
                .with_tenant_snapshot(tenant_id, valid)
                .unwrap(),
        );
        let invalid = br#"{"config_version":1,"unknown":true}"#;
        if let Ok(compiled) = compile_runtime_governance_artifact(invalid) {
            let next = snapshots
                .load_full()
                .with_tenant_snapshot(tenant_id, compiled)
                .unwrap();
            snapshots.store(Arc::new(next));
        }

        assert_eq!(
            policy_effect(
                &snapshots.load().snapshot_for(tenant_id).unwrap(),
                tenant_id
            ),
            PolicyEffect::Allow
        );
    }

    #[test]
    fn governance_artifact_schema_round_trips_and_denies_unknown_fields() {
        let bytes = serde_json::to_vec(&RuntimePolicyGovernanceSettings::default()).unwrap();
        assert!(compile_runtime_governance_artifact(&bytes).is_ok());
        assert!(compile_runtime_governance_artifact(br#"{"unknown":true}"#).is_err());
    }

    #[test]
    fn documented_governance_policy_example_compiles() {
        let snapshot = compile_runtime_governance_artifact(include_bytes!(
            "../../../deploy/governance-policy.example.json"
        ))
        .unwrap();

        assert_eq!(
            snapshot.config.mode,
            prodex_config::GovernanceMode::EnterpriseObserve
        );
    }
}
