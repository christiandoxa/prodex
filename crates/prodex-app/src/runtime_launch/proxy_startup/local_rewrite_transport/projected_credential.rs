use super::super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeProjectedProviderCredential,
};
use super::super::local_rewrite_options::{RuntimeGatewaySecret, RuntimeGatewaySecretSource};
use anyhow::Result;
use prodex_domain::{SecretMaterial, SecretProvider as _, SecretPurpose, SecretResolutionRequest};

const GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED: &str = "gateway outbound secret resolution failed";

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_with_projected_provider_secret<
    T,
>(
    credential: Option<&RuntimeProjectedProviderCredential>,
    expose: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    let credential = credential
        .ok_or_else(|| anyhow::anyhow!("projected provider credential resolution failed"))?;
    let material = credential
        .provider()
        .resolve(&SecretResolutionRequest::new(
            credential.reference().clone(),
            SecretPurpose::ProviderCredential,
        ))
        .map_err(|_| anyhow::anyhow!("projected provider credential resolution failed"))?;
    material.with_exposed_secret(|bytes| {
        let secret = std::str::from_utf8(bytes)
            .map_err(|_| anyhow::anyhow!("projected provider credential resolution failed"))?;
        if secret.is_empty() || secret.chars().any(char::is_whitespace) {
            anyhow::bail!("projected provider credential resolution failed");
        }
        expose(secret)
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_with_outbound_secret<T>(
    secret: &RuntimeGatewaySecret,
    expose: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    match secret.source() {
        RuntimeGatewaySecretSource::Projected {
            credential,
            purpose,
        } => {
            let material = credential
                .provider()
                .resolve(&SecretResolutionRequest::new(
                    credential.reference().clone(),
                    *purpose,
                ))
                .map_err(|_| anyhow::anyhow!(GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED))?;
            expose_secret_material(&material, GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED, expose)
        }
        RuntimeGatewaySecretSource::DevelopmentCompatibility(material) => {
            expose_secret_material(material, GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED, expose)
        }
    }
}

fn expose_secret_material<T>(
    material: &SecretMaterial,
    failure: &'static str,
    expose: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    material.with_exposed_secret(|bytes| {
        let secret = std::str::from_utf8(bytes).map_err(|_| anyhow::anyhow!(failure))?;
        if secret.is_empty() || secret.chars().any(char::is_whitespace) {
            anyhow::bail!(failure);
        }
        expose(secret)
    })
}

pub(super) fn runtime_local_rewrite_apply_projected_bearer(
    shared: &RuntimeLocalRewriteProxyShared,
    request: reqwest::blocking::RequestBuilder,
) -> Result<reqwest::blocking::RequestBuilder> {
    runtime_local_rewrite_with_projected_provider_secret(
        shared.provider_credential.as_ref(),
        |secret| Ok(request.bearer_auth(secret)),
    )
}

pub(super) fn runtime_local_rewrite_apply_projected_header(
    shared: &RuntimeLocalRewriteProxyShared,
    request: reqwest::blocking::RequestBuilder,
    header: &'static str,
) -> Result<reqwest::blocking::RequestBuilder> {
    runtime_local_rewrite_with_projected_provider_secret(
        shared.provider_credential.as_ref(),
        |secret| Ok(request.header(header, secret)),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED, runtime_gateway_with_outbound_secret,
        runtime_local_rewrite_with_projected_provider_secret,
    };
    use crate::runtime_launch::proxy_startup::local_rewrite_options::{
        RuntimeGatewaySecret, RuntimeProjectedProviderCredential,
    };
    use prodex_domain::SecretPurpose;

    fn projected_gateway_secret(
        root: &std::path::Path,
        name: &str,
        purpose: SecretPurpose,
    ) -> RuntimeGatewaySecret {
        let credential = RuntimeProjectedProviderCredential::new(
            prodex_domain::SecretRef::new("external", name, None::<String>),
            secret_store::ProjectedSecretProvider::new(root, "external").unwrap(),
        );
        RuntimeGatewaySecret::projected(credential, purpose)
    }

    fn write_projected_secret(root: &std::path::Path, name: &str, value: &str) {
        std::fs::create_dir_all(root).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join(name);
        std::fs::write(&path, value).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn resolution_failure_is_stable_and_redacted() {
        let root = std::env::temp_dir().join(format!(
            "prodex-provider-resolution-failure-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let credential = RuntimeProjectedProviderCredential::new(
            prodex_domain::SecretRef::new(
                "external",
                "missing-provider-key-secret",
                None::<String>,
            ),
            secret_store::ProjectedSecretProvider::new(&root, "external").unwrap(),
        );

        let error =
            runtime_local_rewrite_with_projected_provider_secret(Some(&credential), |_| Ok(()))
                .unwrap_err();
        let rendered = format!("{error:#}");

        assert_eq!(rendered, "projected provider credential resolution failed");
        assert!(!rendered.contains("missing-provider-key-secret"));
        assert!(!rendered.contains("external"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_secret_reads_rotated_material_on_each_outbound_exposure() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-secret-rotation-{}",
            prodex_domain::RequestId::new()
        ));
        write_projected_secret(&root, "telemetry-token", "old-telemetry-token");
        let secret = projected_gateway_secret(
            &root,
            "telemetry-token",
            SecretPurpose::TelemetryExportCredential,
        );
        runtime_gateway_with_outbound_secret(&secret, |value| {
            assert_eq!(value, "old-telemetry-token");
            Ok(())
        })
        .unwrap();

        write_projected_secret(&root, "telemetry-token", "rotated-telemetry-token");
        runtime_gateway_with_outbound_secret(&secret.clone(), |value| {
            assert_eq!(value, "rotated-telemetry-token");
            Ok(())
        })
        .unwrap();

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn gateway_secret_resolution_failure_is_stable_and_redacted() {
        let root = std::env::temp_dir().join(format!(
            "prodex-gateway-secret-missing-{}",
            prodex_domain::RequestId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let secret = projected_gateway_secret(
            &root,
            "missing-webhook-secret-name",
            SecretPurpose::WebhookSigningSecret,
        );

        let error = runtime_gateway_with_outbound_secret(&secret, |_| Ok(())).unwrap_err();
        let rendered = format!("{error:#}");

        assert_eq!(rendered, GATEWAY_OUTBOUND_SECRET_RESOLUTION_FAILED);
        assert!(!rendered.contains("missing-webhook-secret-name"));
        assert!(!rendered.contains("external"));
        assert!(!rendered.contains(root.to_string_lossy().as_ref()));
        let _ = std::fs::remove_dir_all(root);
    }
}
