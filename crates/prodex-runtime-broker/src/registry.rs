use super::*;
use zeroize::Zeroize;

pub fn runtime_broker_registry_contains_legacy_secrets(mut bytes: Vec<u8>) -> bool {
    let contains = [
        b"\"instance_token\"".as_slice(),
        b"\"admin_token\"".as_slice(),
    ]
    .into_iter()
    .any(|field| bytes.windows(field.len()).any(|window| window == field));
    bytes.zeroize();
    contains
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerRegistry {
    pub pid: u32,
    pub listen_addr: String,
    pub started_at: i64,
    pub upstream_base_url: String,
    pub include_code_review: bool,
    #[serde(default)]
    pub upstream_no_proxy: bool,
    #[serde(default)]
    pub smart_context_enabled: bool,
    pub current_profile: String,
    pub instance_id: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
    #[serde(default)]
    pub openai_mount_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realtime_ws_addr: Option<String>,
}

impl RuntimeBrokerRegistry {
    pub fn admin_url(&self, route: RuntimeBrokerAdminRoute) -> String {
        runtime_broker_admin_url(&self.listen_addr, route)
    }

    pub fn health_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Health)
    }

    pub fn metrics_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Metrics)
    }

    pub fn metrics_prometheus_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::MetricsPrometheus)
    }

    pub fn activate_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::Activate)
    }

    pub fn release_session_affinity_url(&self) -> String {
        self.admin_url(RuntimeBrokerAdminRoute::ReleaseSessionAffinity)
    }

    pub fn matches_launch_config(
        &self,
        upstream_base_url: &str,
        include_code_review: bool,
        upstream_no_proxy: bool,
        smart_context_enabled: bool,
    ) -> bool {
        self.upstream_base_url == upstream_base_url
            && self.include_code_review == include_code_review
            && self.upstream_no_proxy == upstream_no_proxy
            && self.smart_context_enabled == smart_context_enabled
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerHealth {
    pub pid: u32,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub active_requests: usize,
    pub instance_id: String,
    pub persistence_role: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
}

impl RuntimeBrokerHealth {
    pub fn from_metadata(
        metadata: &RuntimeBrokerMetadata,
        pid: u32,
        active_requests: usize,
        persistence_owner: bool,
    ) -> Self {
        Self {
            pid,
            started_at: metadata.started_at,
            current_profile: metadata.current_profile.clone(),
            include_code_review: metadata.include_code_review,
            active_requests,
            instance_id: metadata.instance_id.clone(),
            persistence_role: if persistence_owner {
                "owner"
            } else {
                "follower"
            }
            .to_string(),
            prodex_version: metadata.prodex_version.clone(),
            executable_path: metadata.executable_path.clone(),
            executable_sha256: metadata.executable_sha256.clone(),
        }
    }

    pub fn matches_registry_instance(&self, registry: &RuntimeBrokerRegistry) -> bool {
        self.instance_id == registry.instance_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBrokerAdminRoute {
    Health,
    Metrics,
    MetricsPrometheus,
    Activate,
    ReleaseSessionAffinity,
}

impl RuntimeBrokerAdminRoute {
    pub fn path(self) -> &'static str {
        match self {
            Self::Health => RUNTIME_BROKER_HEALTH_PATH,
            Self::Metrics => RUNTIME_BROKER_METRICS_PATH,
            Self::MetricsPrometheus => RUNTIME_BROKER_METRICS_PROMETHEUS_PATH,
            Self::Activate => RUNTIME_BROKER_ACTIVATE_PATH,
            Self::ReleaseSessionAffinity => RUNTIME_BROKER_RELEASE_SESSION_AFFINITY_PATH,
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            RUNTIME_BROKER_HEALTH_PATH => Some(Self::Health),
            RUNTIME_BROKER_METRICS_PATH => Some(Self::Metrics),
            RUNTIME_BROKER_METRICS_PROMETHEUS_PATH => Some(Self::MetricsPrometheus),
            RUNTIME_BROKER_ACTIVATE_PATH => Some(Self::Activate),
            RUNTIME_BROKER_RELEASE_SESSION_AFFINITY_PATH => Some(Self::ReleaseSessionAffinity),
            _ => None,
        }
    }
}

pub fn runtime_broker_admin_url(listen_addr: &str, route: RuntimeBrokerAdminRoute) -> String {
    format!("http://{}{}", listen_addr, route.path())
}

pub fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.health_url()
}

pub fn runtime_broker_metrics_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.metrics_url()
}

pub fn runtime_broker_metrics_prometheus_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.metrics_prometheus_url()
}

pub fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.activate_url()
}

pub fn runtime_broker_release_session_affinity_url(registry: &RuntimeBrokerRegistry) -> String {
    registry.release_session_affinity_url()
}

pub fn runtime_broker_registry_openai_mount_path(
    registry: &RuntimeBrokerRegistry,
) -> Option<String> {
    registry.openai_mount_path.clone()
}

pub fn runtime_broker_legacy_openai_mount_path(prefix: &str, version: &str) -> String {
    format!("{prefix}{version}")
}

pub fn format_runtime_broker_metrics_targets(targets: &[String]) -> String {
    match targets {
        [] => "-".to_string(),
        [target] => target.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBrokerLaunchConfig<'a> {
    pub upstream_base_url: &'a str,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub smart_context_enabled: bool,
}

impl RuntimeBrokerLaunchConfig<'_> {
    pub fn matches_registry(self, registry: &RuntimeBrokerRegistry) -> bool {
        registry.matches_launch_config(
            self.upstream_base_url,
            self.include_code_review,
            self.upstream_no_proxy,
            self.smart_context_enabled,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerRegistryReuseDecision {
    Reuse,
    LaunchConfigMismatch,
    MissingMatchingHealth,
}

pub fn runtime_broker_registry_reuse_decision(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
    launch_config: RuntimeBrokerLaunchConfig<'_>,
) -> RuntimeBrokerRegistryReuseDecision {
    if !launch_config.matches_registry(registry) {
        return RuntimeBrokerRegistryReuseDecision::LaunchConfigMismatch;
    }
    if health.is_some_and(|health| health.matches_registry_instance(registry)) {
        RuntimeBrokerRegistryReuseDecision::Reuse
    } else {
        RuntimeBrokerRegistryReuseDecision::MissingMatchingHealth
    }
}
