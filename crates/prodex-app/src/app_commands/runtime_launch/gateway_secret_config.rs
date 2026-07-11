use anyhow::{Context as _, Result, bail};
use prodex_domain::{SecretProvider as _, SecretPurpose, SecretRef, SecretResolutionRequest};
use secret_store::ProjectedSecretProvider;
use std::env;

pub(crate) struct GatewaySecretResolver {
    production: bool,
    projected: Option<ProjectedSecretProvider>,
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
        })
    }

    pub(crate) fn production(&self) -> bool {
        self.production
    }

    pub(crate) fn resolve(
        &self,
        context: &str,
        reference: Option<&SecretRef>,
        env_name: Option<&str>,
        direct: Option<&str>,
        purpose: SecretPurpose,
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
            (
                Some(
                    std::str::from_utf8(material.expose_secret())
                        .with_context(|| format!("{context} secret must be UTF-8"))?
                        .to_string(),
                ),
                context.to_string(),
            )
        } else if let Some(value) = direct {
            (Some(value.to_string()), context.to_string())
        } else if let Some(env_name) = env_name {
            if env_name.is_empty() || env_name.chars().any(char::is_whitespace) {
                bail!("{context} must be non-empty without whitespace");
            }
            (
                Some(env::var(env_name).with_context(|| format!("{context} requires {env_name}"))?),
                format!("{context} env {env_name}"),
            )
        } else {
            (None, context.to_string())
        };
        value
            .map(|value| validate_secret_value(&source_context, value))
            .transpose()
    }
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
