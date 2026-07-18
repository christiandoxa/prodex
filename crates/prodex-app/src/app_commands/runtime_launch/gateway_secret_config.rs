use super::{RuntimeGatewaySecret, RuntimeProjectedProviderCredential};
use anyhow::{Context as _, Result, bail};
use prodex_domain::{
    SecretMaterial, SecretProvider as _, SecretPurpose, SecretRef, SecretResolutionRequest,
};
use secret_store::ProjectedSecretProvider;
use sha2::{Digest, Sha256};
use std::env;
use std::sync::Mutex;

pub(crate) struct GatewaySecretResolver {
    production: bool,
    projected: Option<ProjectedSecretProvider>,
    fingerprint: Mutex<Sha256>,
}

impl GatewaySecretResolver {
    pub(crate) fn from_policy(
        policy: &prodex_runtime_policy::RuntimePolicySecretsSettings,
    ) -> Result<Self> {
        let projected = match (
            policy.projected_root.as_deref(),
            policy.projected_provider.as_deref(),
        ) {
            (Some(root), Some(provider)) => Some(
                ProjectedSecretProvider::new(root, provider)
                    .context("failed to initialize projected secret provider")?,
            ),
            (None, None) => None,
            _ => bail!("projected secret provider configuration is incomplete"),
        };
        if policy.production && projected.is_none() {
            bail!("production gateway requires a projected secret provider");
        }
        Ok(Self {
            production: policy.production,
            projected,
            fingerprint: Mutex::new(Sha256::new()),
        })
    }

    pub(crate) fn production(&self) -> bool {
        self.production
    }

    pub(crate) fn projected_provider_credential(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        direct: Option<&str>,
    ) -> Result<Option<RuntimeProjectedProviderCredential>> {
        if reference.is_some() && direct.is_some() {
            bail!("{context} must use exactly one secret source");
        }
        if self.production && direct.is_some() {
            bail!("{context} raw CLI/environment credentials are forbidden in production");
        }
        let Some(reference) = reference else {
            return Ok(None);
        };
        let provider = self
            .projected
            .as_ref()
            .context("projected secret provider is not configured")?;
        if !reference.is_well_formed() || reference.provider() != provider.descriptor().name {
            bail!("{context} secret reference is invalid");
        }
        let mut fingerprint = self
            .fingerprint
            .lock()
            .map_err(|_| anyhow::anyhow!("gateway secret fingerprint state is unavailable"))?;
        fingerprint.update(context.as_bytes());
        fingerprint.update([2]);
        fingerprint.update(reference.provider().as_bytes());
        fingerprint.update([0]);
        fingerprint.update(reference.name().as_bytes());
        fingerprint.update([0]);
        if let Some(version) = reference.version() {
            fingerprint.update(version.as_bytes());
        }
        Ok(Some(RuntimeProjectedProviderCredential::new(
            reference.clone(),
            provider.clone(),
        )))
    }

    pub(crate) fn runtime_secret(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        env_name: Option<&str>,
        direct: Option<&str>,
        purpose: SecretPurpose,
    ) -> Result<Option<RuntimeGatewaySecret>> {
        if reference.is_some() && (env_name.is_some() || direct.is_some()) {
            bail!("{context} must use exactly one secret source");
        }
        if self.production && (env_name.is_some() || direct.is_some()) {
            bail!("{context} raw CLI/environment credentials are forbidden in production");
        }
        if let Some(reference) = reference {
            let credential = self
                .projected_provider_credential(context, Some(reference), None)?
                .expect("projected credential should exist for a secret reference");
            return Ok(Some(RuntimeGatewaySecret::projected(credential, purpose)));
        }
        self.resolve_with_rotation_tracking(context, None, env_name, direct, purpose, true)
            .map(|value| {
                value.map(|value| {
                    RuntimeGatewaySecret::development_compatibility(SecretMaterial::new(
                        value.into_bytes(),
                        None::<String>,
                    ))
                })
            })
    }

    pub(crate) fn resolve(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        env_name: Option<&str>,
        direct: Option<&str>,
        purpose: SecretPurpose,
    ) -> Result<Option<String>> {
        self.resolve_with_rotation_tracking(context, reference, env_name, direct, purpose, true)
    }

    pub(crate) fn resolve_static(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        env_name: Option<&str>,
        direct: Option<&str>,
        purpose: SecretPurpose,
    ) -> Result<Option<String>> {
        self.resolve_with_rotation_tracking(context, reference, env_name, direct, purpose, false)
    }

    pub(crate) fn resolve_static_bytes(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        purpose: SecretPurpose,
    ) -> Result<Option<Vec<u8>>> {
        let Some(reference) = reference else {
            return Ok(None);
        };
        let provider = self
            .projected
            .as_ref()
            .context("projected secret provider is not configured")?;
        if !reference.is_well_formed() || reference.provider() != provider.descriptor().name {
            bail!("{context} secret reference is invalid");
        }
        let material = provider
            .resolve(&SecretResolutionRequest::new(reference.clone(), purpose))
            .map_err(|_| anyhow::anyhow!("{context} secret resolution failed"))?;
        let value = material.with_exposed_secret(<[u8]>::to_vec);
        if value.is_empty() {
            bail!("{context} cannot be empty");
        }
        Ok(Some(value))
    }

    pub(crate) fn resolve_environment_raw(&self, context: &str, env_name: &str) -> Result<String> {
        if self.production {
            bail!("{context} raw CLI/environment credentials are forbidden in production");
        }
        let value = read_environment(context, env_name)?;
        self.track_resolved_value(context, Some(&value))?;
        Ok(value)
    }

    fn resolve_with_rotation_tracking(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        env_name: Option<&str>,
        direct: Option<&str>,
        purpose: SecretPurpose,
        track_rotation: bool,
    ) -> Result<Option<String>> {
        if reference.is_some() && (env_name.is_some() || direct.is_some()) {
            bail!("{context} must use exactly one secret source");
        }
        if self.production && (env_name.is_some() || direct.is_some()) {
            bail!("{context} raw CLI/environment credentials are forbidden in production");
        }
        let (value, source_context) = if let Some(reference) = reference {
            let provider = self
                .projected
                .as_ref()
                .context("projected secret provider is not configured")?;
            let material = provider
                .resolve(&SecretResolutionRequest::new(reference.clone(), purpose))
                .map_err(|_| anyhow::anyhow!("{context} secret resolution failed"))?;
            let value = material
                .with_exposed_secret(|bytes| std::str::from_utf8(bytes).map(str::to_owned))
                .with_context(|| format!("{context} secret must be UTF-8"))?;
            (Some(value), context.to_string())
        } else if let Some(value) = direct {
            (Some(value.to_string()), context.to_string())
        } else if let Some(env_name) = env_name {
            (
                Some(read_environment(context, env_name)?),
                format!("{context} env {env_name}"),
            )
        } else {
            (None, context.to_string())
        };
        let value = value
            .map(|value| validate_secret_value(&source_context, value))
            .transpose()?;
        if track_rotation {
            self.track_resolved_value(context, value.as_deref())?;
        }
        Ok(value)
    }

    fn track_resolved_value(&self, context: &str, value: Option<&str>) -> Result<()> {
        let mut fingerprint = self
            .fingerprint
            .lock()
            .map_err(|_| anyhow::anyhow!("gateway secret fingerprint state is unavailable"))?;
        fingerprint.update(context.as_bytes());
        fingerprint.update([0]);
        match value {
            Some(value) => {
                fingerprint.update([1]);
                fingerprint.update((value.len() as u64).to_le_bytes());
                fingerprint.update(value.as_bytes());
            }
            None => fingerprint.update([0]),
        }
        Ok(())
    }

    pub(crate) fn fingerprint(&self) -> Result<[u8; 32]> {
        let fingerprint = self
            .fingerprint
            .lock()
            .map_err(|_| anyhow::anyhow!("gateway secret fingerprint state is unavailable"))?
            .clone()
            .finalize();
        Ok(fingerprint.into())
    }
}

fn read_environment(context: &str, env_name: &str) -> Result<String> {
    if env_name.is_empty() || env_name.chars().any(char::is_whitespace) {
        bail!("{context} must be non-empty without whitespace");
    }
    env::var(env_name).with_context(|| format!("{context} requires {env_name}"))
}

fn validate_secret_value(context: &str, value: String) -> Result<String> {
    if value.is_empty() {
        bail!("{context} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{context} must not contain whitespace");
    }
    Ok(value)
}

pub(crate) fn gateway_tls_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    secrets: &prodex_runtime_policy::RuntimePolicySecretsSettings,
) -> Result<Option<prodex_gateway_server::GatewayServerTlsConfig>> {
    let workload = &policy.workload_identity;
    if workload.enabled != Some(true) || workload.mtls_required != Some(true) {
        return Ok(None);
    }
    let resolver = GatewaySecretResolver::from_policy(secrets)?;
    let identity = resolver
        .resolve_static_bytes(
            "gateway.workload_identity.tls_identity_ref",
            workload.tls_identity_ref.as_ref(),
            SecretPurpose::DataPlaneCredential,
        )?
        .context("gateway workload TLS identity is required")?;
    let client_ca = resolver
        .resolve_static_bytes(
            "gateway.workload_identity.mtls_ca_ref",
            workload.mtls_ca_ref.as_ref(),
            SecretPurpose::DataPlaneCredential,
        )?
        .context("gateway workload mTLS client CA is required")?;
    prodex_gateway_server::GatewayServerTlsConfig::new(identity, Some(client_ca), true).map(Some)
}
