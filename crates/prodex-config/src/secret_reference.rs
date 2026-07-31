use super::*;

#[derive(Clone, PartialEq, Eq)]
pub enum ConfigSecretSource {
    Reference(SecretRef),
    RawSecretMaterial,
}

impl fmt::Debug for ConfigSecretSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(_) => f.debug_tuple("Reference").field(&"<redacted>").finish(),
            Self::RawSecretMaterial => f.write_str("RawSecretMaterial"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConfigSecretReferencePlan {
    pub tenant_id: TenantId,
    pub reference: SecretRef,
    pub purpose: SecretPurpose,
}

impl fmt::Debug for ConfigSecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigSecretReferencePlan")
            .field("tenant_id", &"<redacted>")
            .field("reference", &self.reference)
            .field("purpose", &self.purpose)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigSecretReferenceError {
    RawSecretMaterialRejected,
    MalformedSecretReference,
}

impl fmt::Display for ConfigSecretReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration secrets must use secret references")
    }
}

impl Error for ConfigSecretReferenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigSecretReferenceErrorStatus {
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSecretReferenceErrorResponsePlan {
    pub status: ConfigSecretReferenceErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_config_secret_reference_error_response(
    error: &ConfigSecretReferenceError,
) -> ConfigSecretReferenceErrorResponsePlan {
    let code = match error {
        ConfigSecretReferenceError::RawSecretMaterialRejected => {
            "configuration_secret_reference_required"
        }
        ConfigSecretReferenceError::MalformedSecretReference => {
            "configuration_secret_reference_invalid"
        }
    };
    ConfigSecretReferenceErrorResponsePlan {
        status: ConfigSecretReferenceErrorStatus::InvalidConfiguration,
        code,
        message: "configuration secrets must use secret references",
    }
}

pub fn plan_config_secret_reference(
    tenant_id: TenantId,
    purpose: SecretPurpose,
    source: ConfigSecretSource,
) -> Result<ConfigSecretReferencePlan, ConfigSecretReferenceError> {
    match source {
        ConfigSecretSource::Reference(reference) => {
            if !reference.is_well_formed() {
                return Err(ConfigSecretReferenceError::MalformedSecretReference);
            }
            Ok(ConfigSecretReferencePlan {
                tenant_id,
                reference,
                purpose,
            })
        }
        ConfigSecretSource::RawSecretMaterial => {
            Err(ConfigSecretReferenceError::RawSecretMaterialRejected)
        }
    }
}
