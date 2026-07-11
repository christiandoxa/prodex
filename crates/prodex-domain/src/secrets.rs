use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::{ZeroizeOnDrop, Zeroizing};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SecretRef {
    provider: String,
    name: String,
    version: Option<String>,
}

impl SecretRef {
    pub fn new(
        provider: impl Into<String>,
        name: impl Into<String>,
        version: Option<impl Into<String>>,
    ) -> Self {
        Self {
            provider: provider.into(),
            name: name.into(),
            version: version.map(Into::into),
        }
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn is_well_formed(&self) -> bool {
        secret_ref_part_is_well_formed(&self.provider)
            && secret_ref_part_is_well_formed(&self.name)
            && self
                .version
                .as_deref()
                .is_none_or(secret_ref_part_is_well_formed)
    }
}

fn secret_ref_part_is_well_formed(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.chars().all(|ch| ch.is_ascii_graphic())
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRef")
            .field("provider", &"<redacted>")
            .field("name", &"<redacted>")
            .field("version", &self.version.as_deref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted-secret-ref>")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretResolutionRequest {
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
}

impl fmt::Debug for SecretResolutionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretResolutionRequest")
            .field("reference", &"<redacted>")
            .field("purpose", &self.purpose)
            .finish()
    }
}

impl SecretResolutionRequest {
    pub fn new(reference: SecretRef, purpose: SecretPurpose) -> Self {
        Self { reference, purpose }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretPurpose {
    DataPlaneCredential,
    ControlPlaneCredential,
    ProviderCredential,
    TelemetryExportCredential,
    OidcClientSecret,
    WebhookSigningSecret,
    BreakGlassCredential,
}

/// Owned secret bytes are erased on drop and can only be borrowed inside a
/// caller-provided closure.
///
/// ```compile_fail
/// use prodex_domain::SecretMaterial;
/// let material = SecretMaterial::new("secret", None::<String>);
/// let _copy = material.clone();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<prodex_domain::SecretMaterial>();
/// ```
pub struct SecretMaterial {
    bytes: Zeroizing<Vec<u8>>,
    version: Option<Zeroizing<String>>,
}

impl ZeroizeOnDrop for SecretMaterial {}

impl SecretMaterial {
    pub fn new(bytes: impl Into<Vec<u8>>, version: Option<impl Into<String>>) -> Self {
        Self {
            bytes: Zeroizing::new(bytes.into()),
            version: version.map(|value| Zeroizing::new(value.into())),
        }
    }

    pub fn with_exposed_secret<T>(&self, expose: impl FnOnce(&[u8]) -> T) -> T {
        expose(self.bytes.as_slice())
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_ref().map(|version| version.as_str())
    }
}

impl fmt::Debug for SecretMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretMaterial")
            .field("bytes", &"<redacted>")
            .field("version", &self.version.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Display for SecretMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<redacted-secret>")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretProviderKind {
    DevelopmentEnvFile,
    ExternalSecretManager,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretProviderDescriptor {
    pub kind: SecretProviderKind,
    pub name: String,
    pub supports_rotation_without_restart: bool,
}

impl fmt::Debug for SecretProviderDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretProviderDescriptor")
            .field("kind", &self.kind)
            .field("name", &"<redacted>")
            .field(
                "supports_rotation_without_restart",
                &self.supports_rotation_without_restart,
            )
            .finish()
    }
}

impl SecretProviderDescriptor {
    pub fn development(name: impl Into<String>) -> Self {
        Self {
            kind: SecretProviderKind::DevelopmentEnvFile,
            name: name.into(),
            supports_rotation_without_restart: false,
        }
    }

    pub fn external(name: impl Into<String>) -> Self {
        Self {
            kind: SecretProviderKind::ExternalSecretManager,
            name: name.into(),
            supports_rotation_without_restart: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRotationPolicy {
    pub max_age_seconds: u64,
    pub overlap_seconds: u64,
    pub requires_audit_event: bool,
}

impl fmt::Debug for SecretRotationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRotationPolicy")
            .field("max_age_seconds", &"<redacted>")
            .field("overlap_seconds", &"<redacted>")
            .field("requires_audit_event", &self.requires_audit_event)
            .finish()
    }
}

impl SecretRotationPolicy {
    pub fn short_lived(max_age_seconds: u64) -> Self {
        Self {
            max_age_seconds,
            overlap_seconds: max_age_seconds / 10,
            requires_audit_event: true,
        }
    }

    pub fn validate(&self) -> Result<(), SecretRotationPolicyError> {
        if self.max_age_seconds == 0 {
            return Err(SecretRotationPolicyError::ZeroMaxAge);
        }
        if self.overlap_seconds >= self.max_age_seconds {
            return Err(SecretRotationPolicyError::OverlapNotShorterThanMaxAge);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretRotationPolicyError {
    ZeroMaxAge,
    OverlapNotShorterThanMaxAge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretErrorStatus {
    InvalidRequest,
    Forbidden,
    NotFound,
    ServiceUnavailable,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretErrorResponsePlan {
    pub status: SecretErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_secret_rotation_policy_error_response(
    error: &SecretRotationPolicyError,
) -> SecretErrorResponsePlan {
    let code = match error {
        SecretRotationPolicyError::ZeroMaxAge => "secret_rotation_max_age_invalid",
        SecretRotationPolicyError::OverlapNotShorterThanMaxAge => "secret_rotation_overlap_invalid",
    };

    SecretErrorResponsePlan {
        status: SecretErrorStatus::InvalidRequest,
        code,
        message: "secret rotation policy is invalid",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRotationStatus {
    pub active: SecretRef,
    pub previous: Option<SecretRef>,
    pub next_refresh_after_epoch_seconds: Option<u64>,
}

impl fmt::Debug for SecretRotationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretRotationStatus")
            .field("active", &"<redacted>")
            .field("has_previous", &self.previous.is_some())
            .field(
                "next_refresh_after_epoch_seconds",
                &self.next_refresh_after_epoch_seconds.map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl SecretRotationStatus {
    pub fn active_only(active: SecretRef) -> Self {
        Self {
            active,
            previous: None,
            next_refresh_after_epoch_seconds: None,
        }
    }

    pub fn rotate_to(
        &self,
        active: SecretRef,
        next_refresh_after_epoch_seconds: Option<u64>,
    ) -> Self {
        Self {
            active,
            previous: Some(self.active.clone()),
            next_refresh_after_epoch_seconds,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretResolutionError {
    NotFound,
    PermissionDenied,
    ProviderUnavailable,
    StaleVersion,
}

pub fn plan_secret_resolution_error_response(
    error: &SecretResolutionError,
) -> SecretErrorResponsePlan {
    match error {
        SecretResolutionError::NotFound => SecretErrorResponsePlan {
            status: SecretErrorStatus::NotFound,
            code: "secret_not_found",
            message: "secret could not be resolved",
        },
        SecretResolutionError::PermissionDenied => SecretErrorResponsePlan {
            status: SecretErrorStatus::Forbidden,
            code: "secret_permission_denied",
            message: "secret could not be resolved",
        },
        SecretResolutionError::ProviderUnavailable => SecretErrorResponsePlan {
            status: SecretErrorStatus::ServiceUnavailable,
            code: "secret_provider_unavailable",
            message: "secret provider is unavailable",
        },
        SecretResolutionError::StaleVersion => SecretErrorResponsePlan {
            status: SecretErrorStatus::Conflict,
            code: "secret_version_stale",
            message: "secret version is stale",
        },
    }
}

pub trait SecretProvider {
    fn descriptor(&self) -> SecretProviderDescriptor;

    fn resolve(
        &self,
        request: &SecretResolutionRequest,
    ) -> Result<SecretMaterial, SecretResolutionError>;
}
