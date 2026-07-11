use super::*;
use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;
use zeroize::Zeroizing;

pub const RUNTIME_BROKER_BOOTSTRAP_VERSION: u16 = 1;
pub const RUNTIME_BROKER_BOOTSTRAP_MAX_BYTES: usize = 64 * 1024;
pub const RUNTIME_BROKER_CAPABILITY_VERSION: u16 = 1;
pub const RUNTIME_BROKER_CAPABILITY_MAX_BYTES: usize = 8 * 1024;
const RUNTIME_BROKER_SECRET_MAX_BYTES: usize = 4 * 1024;

pub struct RuntimeBrokerSecret(Zeroizing<String>);

impl zeroize::ZeroizeOnDrop for RuntimeBrokerSecret {}

impl RuntimeBrokerSecret {
    pub fn new(value: impl Into<String>) -> Result<Self, RuntimeBrokerSecretError> {
        let value = value.into();
        if value.is_empty() || value.len() > RUNTIME_BROKER_SECRET_MAX_BYTES {
            drop(Zeroizing::new(value));
            return Err(RuntimeBrokerSecretError);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    pub fn from_utf8(bytes: Vec<u8>) -> Result<Self, RuntimeBrokerSecretError> {
        let value = String::from_utf8(bytes).map_err(|error| {
            drop(Zeroizing::new(error.into_bytes()));
            RuntimeBrokerSecretError
        })?;
        Self::new(value)
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn matches(&self, candidate: &str) -> bool {
        runtime_broker_constant_time_eq(self.0.as_bytes(), candidate.as_bytes())
    }
}

impl fmt::Debug for RuntimeBrokerSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeBrokerSecret")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeBrokerSecretError;

impl fmt::Display for RuntimeBrokerSecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("runtime broker secret is invalid")
    }
}

impl std::error::Error for RuntimeBrokerSecretError {}

pub fn runtime_broker_constant_time_eq(expected: &[u8], candidate: &[u8]) -> bool {
    let mut difference = expected.len() ^ candidate.len();
    for (index, expected_byte) in expected.iter().enumerate() {
        difference |= usize::from(*expected_byte ^ candidate.get(index).copied().unwrap_or(0));
    }
    difference == 0
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBrokerObservation {
    pub broker_key: String,
    pub listen_addr: String,
    pub metrics: RuntimeBrokerMetrics,
}

#[derive(Clone)]
pub struct RuntimeBrokerMetadata {
    pub broker_key: String,
    pub listen_addr: String,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub instance_id: String,
    pub admin_token: Arc<RuntimeBrokerSecret>,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

impl fmt::Debug for RuntimeBrokerMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeBrokerMetadata")
            .field("broker_key", &self.broker_key)
            .field("listen_addr", &self.listen_addr)
            .field("started_at", &self.started_at)
            .field("current_profile", &self.current_profile)
            .field("include_code_review", &self.include_code_review)
            .field("upstream_no_proxy", &self.upstream_no_proxy)
            .field("instance_id", &self.instance_id)
            .field("admin_token", &"<redacted>")
            .field("prodex_version", &self.prodex_version)
            .field("executable_path", &self.executable_path)
            .field("executable_sha256", &self.executable_sha256)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct RuntimeBrokerSpawnConfig<'a> {
    pub current_profile: &'a str,
    pub upstream_base_url: &'a str,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub smart_context_enabled: bool,
    pub model_context_window_tokens: Option<u64>,
    pub broker_key: &'a str,
    pub instance_id: &'a str,
    pub admin_token: &'a RuntimeBrokerSecret,
    pub listen_addr: Option<&'a str>,
}

impl fmt::Debug for RuntimeBrokerSpawnConfig<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeBrokerSpawnConfig")
            .field("current_profile", &self.current_profile)
            .field("upstream_base_url", &self.upstream_base_url)
            .field("include_code_review", &self.include_code_review)
            .field("upstream_no_proxy", &self.upstream_no_proxy)
            .field("smart_context_enabled", &self.smart_context_enabled)
            .field(
                "model_context_window_tokens",
                &self.model_context_window_tokens,
            )
            .field("broker_key", &self.broker_key)
            .field("instance_id", &self.instance_id)
            .field("admin_token", &"<redacted>")
            .field("listen_addr", &self.listen_addr)
            .finish()
    }
}

pub struct RuntimeBrokerBootstrap {
    pub current_profile: String,
    pub upstream_base_url: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub smart_context_enabled: bool,
    pub model_context_window_tokens: Option<u64>,
    pub broker_key: String,
    pub instance_id: String,
    pub admin_token: Arc<RuntimeBrokerSecret>,
    pub listen_addr: Option<String>,
}

pub struct RuntimeBrokerCapability {
    pub instance_id: String,
    pub admin_token: RuntimeBrokerSecret,
}

impl fmt::Debug for RuntimeBrokerCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeBrokerCapability")
            .field("instance_id", &self.instance_id)
            .field("admin_token", &"<redacted>")
            .finish()
    }
}

impl fmt::Debug for RuntimeBrokerBootstrap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeBrokerBootstrap")
            .field("current_profile", &self.current_profile)
            .field("upstream_base_url", &self.upstream_base_url)
            .field("include_code_review", &self.include_code_review)
            .field("upstream_no_proxy", &self.upstream_no_proxy)
            .field("smart_context_enabled", &self.smart_context_enabled)
            .field(
                "model_context_window_tokens",
                &self.model_context_window_tokens,
            )
            .field("broker_key", &self.broker_key)
            .field("instance_id", &self.instance_id)
            .field("admin_token", &"<redacted>")
            .field("listen_addr", &self.listen_addr)
            .finish()
    }
}

#[derive(Serialize)]
struct RuntimeBrokerBootstrapRef<'a> {
    version: u16,
    current_profile: &'a str,
    upstream_base_url: &'a str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
    broker_key: &'a str,
    instance_id: &'a str,
    admin_token: &'a str,
    listen_addr: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeBrokerBootstrapOwned {
    version: u16,
    current_profile: String,
    upstream_base_url: String,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
    broker_key: String,
    instance_id: String,
    #[serde(deserialize_with = "deserialize_runtime_broker_secret")]
    admin_token: RuntimeBrokerSecret,
    listen_addr: Option<String>,
}

#[derive(Serialize)]
struct RuntimeBrokerCapabilityRef<'a> {
    version: u16,
    instance_id: &'a str,
    admin_token: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeBrokerCapabilityOwned {
    version: u16,
    instance_id: String,
    #[serde(deserialize_with = "deserialize_runtime_broker_secret")]
    admin_token: RuntimeBrokerSecret,
}

fn deserialize_runtime_broker_secret<'de, D>(
    deserializer: D,
) -> Result<RuntimeBrokerSecret, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SecretVisitor;

    impl serde::de::Visitor<'_> for SecretVisitor {
        type Value = RuntimeBrokerSecret;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a valid runtime broker secret")
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            RuntimeBrokerSecret::new(value).map_err(E::custom)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            RuntimeBrokerSecret::new(value).map_err(E::custom)
        }
    }

    deserializer.deserialize_string(SecretVisitor)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeBrokerBootstrapError {
    Io,
    Invalid,
    TooLarge,
    UnsupportedVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeBrokerCapabilityError {
    Io,
    Invalid,
    TooLarge,
    UnsupportedVersion,
}

impl fmt::Display for RuntimeBrokerCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Io => "runtime broker capability I/O failed",
            Self::Invalid => "runtime broker capability is invalid",
            Self::TooLarge => "runtime broker capability exceeds the size limit",
            Self::UnsupportedVersion => "runtime broker capability version is unsupported",
        })
    }
}

impl std::error::Error for RuntimeBrokerCapabilityError {}

impl fmt::Display for RuntimeBrokerBootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Io => "runtime broker bootstrap I/O failed",
            Self::Invalid => "runtime broker bootstrap is invalid",
            Self::TooLarge => "runtime broker bootstrap exceeds the size limit",
            Self::UnsupportedVersion => "runtime broker bootstrap version is unsupported",
        })
    }
}

impl std::error::Error for RuntimeBrokerBootstrapError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerProcessCommandPlan {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub environment: Vec<(OsString, OsString)>,
}

pub fn runtime_broker_process_args() -> Vec<OsString> {
    vec![OsString::from("__runtime-broker")]
}

pub fn runtime_broker_process_command_plan(
    executable: impl Into<PathBuf>,
    prodex_home: impl Into<PathBuf>,
) -> RuntimeBrokerProcessCommandPlan {
    let prodex_home = prodex_home.into();
    RuntimeBrokerProcessCommandPlan {
        executable: executable.into(),
        args: runtime_broker_process_args(),
        environment: vec![(OsString::from("PRODEX_HOME"), prodex_home.into_os_string())],
    }
}

pub fn write_runtime_broker_bootstrap(
    mut writer: impl Write,
    config: RuntimeBrokerSpawnConfig<'_>,
) -> Result<(), RuntimeBrokerBootstrapError> {
    validate_runtime_broker_bootstrap_fields(
        config.current_profile,
        config.upstream_base_url,
        config.broker_key,
        config.instance_id,
        config.listen_addr,
    )?;
    let wire = RuntimeBrokerBootstrapRef {
        version: RUNTIME_BROKER_BOOTSTRAP_VERSION,
        current_profile: config.current_profile,
        upstream_base_url: config.upstream_base_url,
        include_code_review: config.include_code_review,
        upstream_no_proxy: config.upstream_no_proxy,
        smart_context_enabled: config.smart_context_enabled,
        model_context_window_tokens: config.model_context_window_tokens,
        broker_key: config.broker_key,
        instance_id: config.instance_id,
        admin_token: config.admin_token.expose(),
        listen_addr: config.listen_addr,
    };
    let payload = Zeroizing::new(
        serde_json::to_vec(&wire).map_err(|_| RuntimeBrokerBootstrapError::Invalid)?,
    );
    if payload.len() > RUNTIME_BROKER_BOOTSTRAP_MAX_BYTES {
        return Err(RuntimeBrokerBootstrapError::TooLarge);
    }
    writer
        .write_all(payload.as_slice())
        .map_err(|_| RuntimeBrokerBootstrapError::Io)
}

pub fn read_runtime_broker_bootstrap(
    reader: impl Read,
) -> Result<RuntimeBrokerBootstrap, RuntimeBrokerBootstrapError> {
    let mut payload = Zeroizing::new(Vec::new());
    reader
        .take(RUNTIME_BROKER_BOOTSTRAP_MAX_BYTES as u64 + 1)
        .read_to_end(&mut payload)
        .map_err(|_| RuntimeBrokerBootstrapError::Io)?;
    if payload.len() > RUNTIME_BROKER_BOOTSTRAP_MAX_BYTES {
        return Err(RuntimeBrokerBootstrapError::TooLarge);
    }
    let wire: RuntimeBrokerBootstrapOwned =
        serde_json::from_slice(&payload).map_err(|_| RuntimeBrokerBootstrapError::Invalid)?;
    if wire.version != RUNTIME_BROKER_BOOTSTRAP_VERSION {
        return Err(RuntimeBrokerBootstrapError::UnsupportedVersion);
    }
    validate_runtime_broker_bootstrap_fields(
        &wire.current_profile,
        &wire.upstream_base_url,
        &wire.broker_key,
        &wire.instance_id,
        wire.listen_addr.as_deref(),
    )?;
    Ok(RuntimeBrokerBootstrap {
        current_profile: wire.current_profile,
        upstream_base_url: wire.upstream_base_url,
        include_code_review: wire.include_code_review,
        upstream_no_proxy: wire.upstream_no_proxy,
        smart_context_enabled: wire.smart_context_enabled,
        model_context_window_tokens: wire.model_context_window_tokens,
        broker_key: wire.broker_key,
        instance_id: wire.instance_id,
        admin_token: Arc::new(wire.admin_token),
        listen_addr: wire.listen_addr,
    })
}

pub fn write_runtime_broker_capability(
    mut writer: impl Write,
    instance_id: &str,
    admin_token: &RuntimeBrokerSecret,
) -> Result<(), RuntimeBrokerCapabilityError> {
    if !runtime_broker_instance_id_is_valid(instance_id) {
        return Err(RuntimeBrokerCapabilityError::Invalid);
    }
    let payload = Zeroizing::new(
        serde_json::to_vec(&RuntimeBrokerCapabilityRef {
            version: RUNTIME_BROKER_CAPABILITY_VERSION,
            instance_id,
            admin_token: admin_token.expose(),
        })
        .map_err(|_| RuntimeBrokerCapabilityError::Invalid)?,
    );
    if payload.len() > RUNTIME_BROKER_CAPABILITY_MAX_BYTES {
        return Err(RuntimeBrokerCapabilityError::TooLarge);
    }
    writer
        .write_all(payload.as_slice())
        .map_err(|_| RuntimeBrokerCapabilityError::Io)
}

pub fn read_runtime_broker_capability(
    reader: impl Read,
) -> Result<RuntimeBrokerCapability, RuntimeBrokerCapabilityError> {
    let mut payload = Zeroizing::new(Vec::new());
    reader
        .take(RUNTIME_BROKER_CAPABILITY_MAX_BYTES as u64 + 1)
        .read_to_end(&mut payload)
        .map_err(|_| RuntimeBrokerCapabilityError::Io)?;
    if payload.len() > RUNTIME_BROKER_CAPABILITY_MAX_BYTES {
        return Err(RuntimeBrokerCapabilityError::TooLarge);
    }
    let wire: RuntimeBrokerCapabilityOwned =
        serde_json::from_slice(&payload).map_err(|_| RuntimeBrokerCapabilityError::Invalid)?;
    if wire.version != RUNTIME_BROKER_CAPABILITY_VERSION {
        return Err(RuntimeBrokerCapabilityError::UnsupportedVersion);
    }
    if !runtime_broker_instance_id_is_valid(&wire.instance_id) {
        return Err(RuntimeBrokerCapabilityError::Invalid);
    }
    Ok(RuntimeBrokerCapability {
        instance_id: wire.instance_id,
        admin_token: wire.admin_token,
    })
}

fn validate_runtime_broker_bootstrap_fields(
    current_profile: &str,
    upstream_base_url: &str,
    broker_key: &str,
    instance_id: &str,
    listen_addr: Option<&str>,
) -> Result<(), RuntimeBrokerBootstrapError> {
    let valid = !current_profile.is_empty()
        && current_profile.len() <= 256
        && !upstream_base_url.is_empty()
        && upstream_base_url.len() <= 4096
        && !broker_key.is_empty()
        && broker_key.len() <= 256
        && broker_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        && runtime_broker_instance_id_is_valid(instance_id)
        && listen_addr.is_none_or(|value| !value.is_empty() && value.len() <= 256);
    if valid {
        Ok(())
    } else {
        Err(RuntimeBrokerBootstrapError::Invalid)
    }
}

fn runtime_broker_instance_id_is_valid(instance_id: &str) -> bool {
    !instance_id.is_empty()
        && instance_id.len() <= 256
        && instance_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub fn runtime_broker_startup_grace_seconds(ready_timeout_ms: u64, idle_grace_seconds: i64) -> i64 {
    let ready_timeout_seconds = ready_timeout_ms.div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(idle_grace_seconds)
}
