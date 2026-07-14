use prodex_domain::{
    CredentialScope, FindingKind, InspectionCoverage, ModelCapability, PolicyRevisionId,
    PrincipalKind, Role, SecretRef, TenantId,
};
use secret_store::SecretBackendKind;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::path::PathBuf;

pub const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub const PRODEX_POLICY_VERSION: u32 = 1;
pub const PRODEX_RUNTIME_PROXY_PRESET_ENV: &str = "PRODEX_RUNTIME_PROXY_PRESET";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePolicyServiceMode {
    #[default]
    Gateway,
    ControlPlane,
}

impl RuntimePolicyServiceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::ControlPlane => "control-plane",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeLogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernanceMode {
    #[default]
    Personal,
    EnterpriseObserve,
    EnterpriseEnforce,
    BankEnforce,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernanceRolloutMode {
    #[default]
    Off,
    Observe,
    Enforce,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernanceDataClassification {
    Public,
    #[default]
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernanceProviderTrustTier {
    #[default]
    Standard,
    Enterprise,
    RestrictedApproved,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernanceUnknownClassificationBehavior {
    #[default]
    UseDefault,
    Deny,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyFailureMode {
    #[default]
    Open,
    Closed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGovernanceSessionSettings {
    pub absolute_timeout_seconds: Option<u32>,
    pub idle_timeout_seconds: Option<u32>,
    pub max_concurrent: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGovernanceProviderSettings {
    pub descriptor_revision: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub revoked: bool,
    pub trust_tier: RuntimeGovernanceProviderTrustTier,
    #[serde(default)]
    pub local_execution: bool,
    pub maximum_classification: RuntimeGovernanceDataClassification,
    #[serde(default)]
    pub regions: Vec<String>,
    #[serde(default)]
    pub retention_seconds: u32,
    #[serde(default)]
    pub training_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyInspectionPattern {
    pub tenant_id: TenantId,
    pub id: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyChannel {
    Cli,
    Ide,
    Api,
    Mcp,
    InternalService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyAction {
    InvokeModel,
    UseTool,
    UploadContent,
    CompactContext,
    MutateControlPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyRequestRisk {
    Low,
    Elevated,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyNetworkZone {
    Local,
    TrustedInternal,
    Partner,
    Public,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyEffect {
    Allow,
    RequireApproval,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyDataModality {
    Text,
    Image,
    Audio,
    Video,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGovernancePolicyAuditDetailLevel {
    Minimal,
    Standard,
    Elevated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGovernancePolicyRuleCondition {
    pub channel: Option<RuntimeGovernancePolicyChannel>,
    pub principal_kind: Option<PrincipalKind>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub minimum_role: Option<Role>,
    pub credential_scope: Option<CredentialScope>,
    pub action: Option<RuntimeGovernancePolicyAction>,
    pub route: Option<String>,
    pub minimum_classification: Option<RuntimeGovernanceDataClassification>,
    pub inspection_coverage: Option<InspectionCoverage>,
    pub minimum_request_risk: Option<RuntimeGovernancePolicyRequestRisk>,
    pub network_zone: Option<RuntimeGovernancePolicyNetworkZone>,
    pub maximum_session_age_seconds: Option<u64>,
    pub maximum_session_idle_seconds: Option<u64>,
    pub session_revoked: Option<bool>,
    pub session_mfa_satisfied: Option<bool>,
    pub minimum_session_retained_classification: Option<RuntimeGovernanceDataClassification>,
    pub minimum_authentication_strength: Option<u8>,
    pub environment_mfa_satisfied: Option<bool>,
    pub requested_capability: Option<ModelCapability>,
    pub requested_model: Option<String>,
    pub requested_tool: Option<String>,
    pub requested_modality: Option<RuntimeGovernancePolicyDataModality>,
    pub break_glass_required: Option<bool>,
    pub break_glass_scope: Option<String>,
    pub quota_has_headroom: Option<bool>,
    pub quota_reservation_required: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeGovernancePolicyObligation {
    MaskFinding {
        finding_kind: FindingKind,
    },
    MinimumProviderTrust {
        trust_tier: RuntimeGovernanceProviderTrustTier,
    },
    AllowProvider {
        selector: String,
    },
    DenyProvider {
        selector: String,
    },
    RequireLocalExecution,
    ProhibitRetention,
    ProhibitTrainingUse,
    RequireRegion {
        selector: String,
    },
    DisableTools,
    AllowTool {
        selector: String,
    },
    AllowModel {
        selector: String,
    },
    AllowModality {
        modality: RuntimeGovernancePolicyDataModality,
    },
    MaxInputTokens {
        value: u32,
    },
    MaxOutputTokens {
        value: u32,
    },
    MaxContextTokens {
        value: u32,
    },
    RequireResponseInspection,
    SessionIdleTimeoutSeconds {
        value: u32,
    },
    SessionAbsoluteTimeoutSeconds {
        value: u32,
    },
    RequireReauthentication,
    RequireMfa,
    AuditDetail {
        level: RuntimeGovernancePolicyAuditDetailLevel,
    },
    RequireHumanApproval,
    RetentionSeconds {
        value: u32,
    },
    DenyFallbackOutsideEligibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGovernancePolicyRule {
    pub id: String,
    pub condition: RuntimeGovernancePolicyRuleCondition,
    pub effect: RuntimeGovernancePolicyEffect,
    pub obligations: Vec<RuntimeGovernancePolicyObligation>,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGovernanceSettings {
    #[serde(default = "default_governance_config_version")]
    pub config_version: u32,
    #[serde(default)]
    pub authority_tenants: Vec<TenantId>,
    #[serde(default)]
    pub mode: RuntimeGovernanceMode,
    #[serde(default)]
    pub inspection: RuntimeGovernanceRolloutMode,
    #[serde(default)]
    pub classification: RuntimeGovernanceRolloutMode,
    #[serde(default)]
    pub policy: RuntimeGovernanceRolloutMode,
    #[serde(default)]
    pub routing: RuntimeGovernanceRolloutMode,
    #[serde(default)]
    pub mandatory_audit: bool,
    #[serde(default = "default_true")]
    pub anonymous_data_plane: bool,
    #[serde(default = "default_true")]
    pub raw_secret_sources: bool,
    #[serde(default)]
    pub policy_revision: Option<PolicyRevisionId>,
    #[serde(default)]
    pub policy_valid_until_unix_ms: Option<u64>,
    #[serde(default)]
    pub classification_revision: Option<String>,
    #[serde(default)]
    pub classification_checksum: Option<String>,
    #[serde(default)]
    pub provider_registry_revision: Option<u64>,
    #[serde(default)]
    pub routing_score_revision: Option<u64>,
    #[serde(default)]
    pub provider: Option<RuntimePolicyGovernanceProviderSettings>,
    #[serde(default)]
    pub inspection_patterns: Vec<RuntimePolicyInspectionPattern>,
    #[serde(default)]
    pub classification_default: RuntimeGovernanceDataClassification,
    #[serde(default)]
    pub classification_unknown: RuntimeGovernanceUnknownClassificationBehavior,
    #[serde(default)]
    pub policy_failure_mode: RuntimeGovernancePolicyFailureMode,
    #[serde(default)]
    pub active_policy_revision: Option<PolicyRevisionId>,
    #[serde(default)]
    pub policy_rules: Vec<RuntimeGovernancePolicyRule>,
    #[serde(default)]
    pub session: RuntimePolicyGovernanceSessionSettings,
}

impl Default for RuntimePolicyGovernanceSettings {
    fn default() -> Self {
        Self {
            config_version: 1,
            authority_tenants: Vec::new(),
            mode: RuntimeGovernanceMode::Personal,
            inspection: RuntimeGovernanceRolloutMode::Off,
            classification: RuntimeGovernanceRolloutMode::Off,
            policy: RuntimeGovernanceRolloutMode::Off,
            routing: RuntimeGovernanceRolloutMode::Off,
            mandatory_audit: false,
            anonymous_data_plane: true,
            raw_secret_sources: true,
            policy_revision: None,
            policy_valid_until_unix_ms: None,
            classification_revision: None,
            classification_checksum: None,
            provider_registry_revision: None,
            routing_score_revision: None,
            provider: None,
            inspection_patterns: Vec::new(),
            classification_default: RuntimeGovernanceDataClassification::Internal,
            classification_unknown: RuntimeGovernanceUnknownClassificationBehavior::UseDefault,
            policy_failure_mode: RuntimeGovernancePolicyFailureMode::Open,
            active_policy_revision: None,
            policy_rules: Vec::new(),
            session: RuntimePolicyGovernanceSessionSettings::default(),
        }
    }
}

const fn default_governance_config_version() -> u32 {
    1
}

const fn default_true() -> bool {
    true
}

impl RuntimeLogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimePolicySummary {
    pub path: PathBuf,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct RuntimePolicyConfig {
    pub path: PathBuf,
    pub version: u32,
    pub service_mode: RuntimePolicyServiceMode,
    pub runtime: RuntimePolicyRuntimeSettings,
    pub runtime_proxy: RuntimePolicyProxySettings,
    pub gateway: RuntimePolicyGatewaySettings,
    pub secrets: RuntimePolicySecretsSettings,
    pub governance: RuntimePolicyGovernanceSettings,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicyRuntimeSettings {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicySecretsSettings {
    pub backend: Option<SecretBackendKind>,
    pub keyring_service: Option<String>,
    pub production: bool,
    pub projected_root: Option<PathBuf>,
    pub projected_provider: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewaySettings {
    pub listen_addr: Option<String>,
    pub expected_host: Option<String>,
    pub restricted_egress: Option<bool>,
    pub replica_count: Option<u16>,
    pub require_multi_replica_accounting_checks: Option<bool>,
    pub provider: Option<String>,
    pub harness: Option<prodex_provider_core::HarnessMode>,
    pub base_url: Option<String>,
    pub require_auth: Option<bool>,
    pub auth_token_ref: Option<SecretRef>,
    pub provider_api_key_ref: Option<SecretRef>,
    #[serde(default)]
    pub adaptive_routing: RuntimePolicyAdaptiveRoutingSettings,
    #[serde(default)]
    pub state: RuntimePolicyGatewayStateSettings,
    #[serde(default)]
    pub admin_tokens: Vec<RuntimePolicyGatewayAdminToken>,
    #[serde(default)]
    pub sso: RuntimePolicyGatewaySsoSettings,
    #[serde(default)]
    pub route_aliases: Vec<RuntimePolicyGatewayRouteAlias>,
    #[serde(default)]
    pub request_constraints: RuntimePolicyGatewayRequestConstraintSettings,
    #[serde(default)]
    pub virtual_keys: Vec<RuntimePolicyGatewayVirtualKey>,
    #[serde(default)]
    pub observability: RuntimePolicyGatewayObservabilitySettings,
    #[serde(default)]
    pub guardrails: RuntimePolicyGatewayGuardrailsSettings,
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
    #[serde(default)]
    pub workload_identity: RuntimePolicyGatewayWorkloadIdentitySettings,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayRequestConstraintSettings {
    pub enabled: Option<bool>,
    pub unknown_context: Option<String>,
    pub safe_window_tokens: Option<u64>,
    pub oversized_output: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyAdaptiveRoutingSettings {
    pub enabled: Option<bool>,
    pub shadow_mode: Option<bool>,
    pub window_size: Option<usize>,
    pub min_samples: Option<u64>,
    pub exploration_rate: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayStateSettings {
    pub backend: Option<String>,
    pub sqlite_path: Option<String>,
    pub postgres_url_env: Option<String>,
    pub redis_url_env: Option<String>,
    pub postgres_url_ref: Option<SecretRef>,
    pub redis_url_ref: Option<SecretRef>,
    pub postgres_tls_mode: Option<String>,
    pub postgres_tls_ca_path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayAdminToken {
    pub name: String,
    #[serde(default)]
    pub token_env: String,
    pub token_ref: Option<SecretRef>,
    pub role: Option<String>,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    #[serde(default)]
    pub allowed_key_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewaySsoSettings {
    pub proxy_token_env: Option<String>,
    pub proxy_token_ref: Option<SecretRef>,
    pub require_tenant: Option<bool>,
    pub token_header: Option<String>,
    pub user_header: Option<String>,
    pub role_header: Option<String>,
    pub tenant_header: Option<String>,
    pub key_prefixes_header: Option<String>,
    pub oidc_issuer: Option<String>,
    pub oidc_audience: Option<String>,
    pub oidc_jwks_url: Option<String>,
    #[serde(default)]
    pub oidc_jwks_origin_allowlist: Vec<String>,
    pub oidc_user_claim: Option<String>,
    pub oidc_role_claim: Option<String>,
    pub oidc_tenant_claim: Option<String>,
    pub oidc_key_prefixes_claim: Option<String>,
    pub default_role: Option<String>,
    pub remote_human: Option<bool>,
    pub required_scope: Option<String>,
    pub authentication_strength: Option<String>,
    pub browser_flow: Option<bool>,
    pub pkce_method: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayWorkloadIdentitySettings {
    pub enabled: Option<bool>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub required_scope: Option<String>,
    pub mtls_required: Option<bool>,
    pub mtls_ca_ref: Option<SecretRef>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayRouteAlias {
    pub alias: String,
    #[serde(default)]
    pub models: Vec<String>,
    pub strategy: Option<String>,
    #[serde(default)]
    pub model_metrics: Vec<RuntimePolicyGatewayRouteModelMetrics>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayRouteModelMetrics {
    pub model: String,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub latency_ms: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayVirtualKey {
    pub name: String,
    #[serde(default)]
    pub token_env: String,
    pub token_ref: Option<SecretRef>,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    pub budget_usd: Option<f64>,
    pub request_budget: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayObservabilitySettings {
    #[serde(default)]
    pub sinks: Vec<String>,
    pub call_id_header: Option<String>,
    pub jsonl_path: Option<String>,
    pub http_endpoint: Option<String>,
    pub http_schema: Option<String>,
    pub http_bearer_token_env: Option<String>,
    pub http_bearer_token_ref: Option<SecretRef>,
    pub siem_endpoint: Option<String>,
    pub siem_bearer_token_ref: Option<SecretRef>,
    pub siem_mtls_identity_ref: Option<SecretRef>,
    pub siem_signing_key_ref: Option<SecretRef>,
    pub siem_max_batch_events: Option<u16>,
    pub siem_max_batch_bytes: Option<u32>,
    pub siem_max_attempts: Option<u8>,
    pub siem_retry_base_ms: Option<u64>,
    pub siem_retry_max_ms: Option<u64>,
    pub siem_max_lag_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyGatewayGuardrailsSettings {
    #[serde(default)]
    pub blocked_keywords: Vec<String>,
    #[serde(default)]
    pub blocked_output_keywords: Vec<String>,
    #[serde(default)]
    pub allowed_models: Vec<String>,
    pub presidio_redaction: Option<bool>,
    pub prompt_injection_detection: Option<bool>,
    pub pii_redaction: Option<bool>,
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub webhook_host_allowlist: Vec<String>,
    #[serde(default)]
    pub webhook_phases: Vec<String>,
    pub webhook_bearer_token_env: Option<String>,
    pub webhook_bearer_token_ref: Option<SecretRef>,
    pub webhook_fail_closed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePolicyProxyPreset {
    Low,
    Default,
    ManyTerminals,
    Aggressive,
}

impl RuntimePolicyProxyPreset {
    pub const VALID_VALUES: &'static [&'static str] =
        &["low", "default", "many-terminals", "aggressive"];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Default => "default",
            Self::ManyTerminals => "many-terminals",
            Self::Aggressive => "aggressive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value != value.trim() {
            return None;
        }
        match value.to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "default" => Some(Self::Default),
            "many-terminals" | "many_terminals" => Some(Self::ManyTerminals),
            "aggressive" => Some(Self::Aggressive),
            _ => None,
        }
    }

    fn settings(self) -> RuntimePolicyProxySettings {
        let preset = RuntimePolicyProxyPresetSelection::selected(self);
        // Presets only tune local concurrency/admission knobs. Transport timeouts stay unset
        // to preserve upstream Codex stream and reconnect behavior.
        match self {
            Self::Low => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(4),
                long_lived_worker_count: Some(8),
                probe_refresh_worker_count: Some(2),
                async_worker_count: Some(2),
                long_lived_queue_capacity: Some(128),
                active_request_limit: Some(48),
                profile_inflight_soft_limit: Some(2),
                profile_inflight_hard_limit: Some(4),
                responses_active_limit: Some(36),
                compact_active_limit: Some(3),
                websocket_active_limit: Some(8),
                standard_active_limit: Some(2),
                websocket_connect_worker_count: Some(4),
                websocket_connect_queue_capacity: Some(32),
                websocket_connect_overflow_capacity: Some(64),
                websocket_dns_worker_count: Some(2),
                websocket_dns_queue_capacity: Some(16),
                websocket_dns_overflow_capacity: Some(32),
                startup_sync_probe_warm_limit: Some(1),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Default => RuntimePolicyProxySettings {
                preset,
                ..RuntimePolicyProxySettings::default()
            },
            Self::ManyTerminals => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(12),
                long_lived_worker_count: Some(32),
                probe_refresh_worker_count: Some(4),
                async_worker_count: Some(4),
                long_lived_queue_capacity: Some(512),
                active_request_limit: Some(160),
                profile_inflight_soft_limit: Some(4),
                profile_inflight_hard_limit: Some(8),
                responses_active_limit: Some(120),
                compact_active_limit: Some(8),
                websocket_active_limit: Some(32),
                standard_active_limit: Some(8),
                websocket_connect_worker_count: Some(12),
                websocket_connect_queue_capacity: Some(96),
                websocket_connect_overflow_capacity: Some(384),
                websocket_dns_worker_count: Some(6),
                websocket_dns_queue_capacity: Some(48),
                websocket_dns_overflow_capacity: Some(96),
                startup_sync_probe_warm_limit: Some(2),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Aggressive => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(24),
                long_lived_worker_count: Some(96),
                probe_refresh_worker_count: Some(8),
                async_worker_count: Some(8),
                long_lived_queue_capacity: Some(1024),
                active_request_limit: Some(384),
                profile_inflight_soft_limit: Some(8),
                profile_inflight_hard_limit: Some(16),
                responses_active_limit: Some(288),
                compact_active_limit: Some(16),
                websocket_active_limit: Some(96),
                standard_active_limit: Some(16),
                websocket_connect_worker_count: Some(16),
                websocket_connect_queue_capacity: Some(128),
                websocket_connect_overflow_capacity: Some(512),
                websocket_dns_worker_count: Some(8),
                websocket_dns_queue_capacity: Some(64),
                websocket_dns_overflow_capacity: Some(128),
                startup_sync_probe_warm_limit: Some(3),
                ..RuntimePolicyProxySettings::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimePolicyProxyPresetSelection(Option<RuntimePolicyProxyPreset>);

impl RuntimePolicyProxyPresetSelection {
    pub fn selected(preset: RuntimePolicyProxyPreset) -> Self {
        Self(Some(preset))
    }

    pub fn get(self) -> Option<RuntimePolicyProxyPreset> {
        self.0
    }
}

impl<'de> Deserialize<'de> for RuntimePolicyProxyPresetSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RuntimePolicyProxyPreset::parse(&value)
            .map(Self::selected)
            .ok_or_else(|| {
                de::Error::unknown_variant(value.as_str(), RuntimePolicyProxyPreset::VALID_VALUES)
            })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyProxySettings {
    #[serde(default)]
    pub preset: RuntimePolicyProxyPresetSelection,
    pub worker_count: Option<usize>,
    pub long_lived_worker_count: Option<usize>,
    pub probe_refresh_worker_count: Option<usize>,
    pub async_worker_count: Option<usize>,
    pub long_lived_queue_capacity: Option<usize>,
    pub active_request_limit: Option<usize>,
    pub profile_inflight_soft_limit: Option<usize>,
    pub profile_inflight_hard_limit: Option<usize>,
    pub responses_active_limit: Option<usize>,
    pub compact_active_limit: Option<usize>,
    pub websocket_active_limit: Option<usize>,
    pub standard_active_limit: Option<usize>,
    pub http_connect_timeout_ms: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub compact_request_timeout_ms: Option<u64>,
    pub sse_lookahead_timeout_ms: Option<u64>,
    pub prefetch_backpressure_retry_ms: Option<u64>,
    pub prefetch_backpressure_timeout_ms: Option<u64>,
    pub prefetch_max_buffered_bytes: Option<usize>,
    pub websocket_connect_timeout_ms: Option<u64>,
    pub websocket_happy_eyeballs_delay_ms: Option<u64>,
    pub websocket_precommit_progress_timeout_ms: Option<u64>,
    pub websocket_connect_worker_count: Option<usize>,
    pub websocket_connect_queue_capacity: Option<usize>,
    pub websocket_connect_overflow_capacity: Option<usize>,
    pub websocket_dns_worker_count: Option<usize>,
    pub websocket_dns_queue_capacity: Option<usize>,
    pub websocket_dns_overflow_capacity: Option<usize>,
    pub broker_ready_timeout_ms: Option<u64>,
    pub broker_health_connect_timeout_ms: Option<u64>,
    pub broker_health_read_timeout_ms: Option<u64>,
    pub websocket_previous_response_reuse_stale_ms: Option<u64>,
    pub admission_wait_budget_ms: Option<u64>,
    pub pressure_admission_wait_budget_ms: Option<u64>,
    pub long_lived_queue_wait_budget_ms: Option<u64>,
    pub pressure_long_lived_queue_wait_budget_ms: Option<u64>,
    pub sync_probe_pressure_pause_ms: Option<u64>,
    pub responses_critical_floor_percent: Option<i64>,
    pub startup_sync_probe_warm_limit: Option<usize>,
}

impl RuntimePolicyProxySettings {
    pub fn preset(&self) -> Option<RuntimePolicyProxyPreset> {
        self.preset.get()
    }

    pub fn with_effective_preset(
        self,
        env_preset: Option<RuntimePolicyProxyPreset>,
    ) -> RuntimePolicyProxySettings {
        let selected_preset = env_preset.or_else(|| self.preset());
        let Some(selected_preset) = selected_preset else {
            return self;
        };

        let mut effective = selected_preset.settings();
        effective.apply_non_preset_overrides(self);
        effective.preset = RuntimePolicyProxyPresetSelection::selected(selected_preset);
        effective
    }

    fn apply_non_preset_overrides(&mut self, overrides: RuntimePolicyProxySettings) {
        macro_rules! apply_optional_overrides {
            ($($field:ident),+ $(,)?) => {
                $(
                    if overrides.$field.is_some() {
                        self.$field = overrides.$field;
                    }
                )+
            };
        }

        apply_optional_overrides!(
            worker_count,
            long_lived_worker_count,
            probe_refresh_worker_count,
            async_worker_count,
            long_lived_queue_capacity,
            active_request_limit,
            profile_inflight_soft_limit,
            profile_inflight_hard_limit,
            responses_active_limit,
            compact_active_limit,
            websocket_active_limit,
            standard_active_limit,
            http_connect_timeout_ms,
            stream_idle_timeout_ms,
            compact_request_timeout_ms,
            sse_lookahead_timeout_ms,
            prefetch_backpressure_retry_ms,
            prefetch_backpressure_timeout_ms,
            prefetch_max_buffered_bytes,
            websocket_connect_timeout_ms,
            websocket_happy_eyeballs_delay_ms,
            websocket_precommit_progress_timeout_ms,
            websocket_connect_worker_count,
            websocket_connect_queue_capacity,
            websocket_connect_overflow_capacity,
            websocket_dns_worker_count,
            websocket_dns_queue_capacity,
            websocket_dns_overflow_capacity,
            broker_ready_timeout_ms,
            broker_health_connect_timeout_ms,
            broker_health_read_timeout_ms,
            websocket_previous_response_reuse_stale_ms,
            admission_wait_budget_ms,
            pressure_admission_wait_budget_ms,
            long_lived_queue_wait_budget_ms,
            pressure_long_lived_queue_wait_budget_ms,
            sync_probe_pressure_pause_ms,
            responses_critical_floor_percent,
            startup_sync_probe_warm_limit,
        );
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyFile {
    pub version: u32,
    #[serde(default)]
    pub service_mode: RuntimePolicyServiceMode,
    #[serde(default)]
    pub runtime: RuntimePolicyRuntimeFile,
    #[serde(default)]
    pub runtime_proxy: RuntimePolicyProxySettings,
    #[serde(default)]
    pub gateway: RuntimePolicyGatewaySettings,
    #[serde(default)]
    pub secrets: RuntimePolicySecretsFile,
    #[serde(default)]
    pub governance: RuntimePolicyGovernanceSettings,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicyRuntimeFile {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePolicySecretsFile {
    pub backend: Option<String>,
    pub keyring_service: Option<String>,
    #[serde(default)]
    pub production: bool,
    pub projected_root: Option<String>,
    pub projected_provider: Option<String>,
}
