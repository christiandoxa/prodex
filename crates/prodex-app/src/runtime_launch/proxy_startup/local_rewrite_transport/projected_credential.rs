use super::super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeProjectedProviderCredential,
};
use anyhow::Result;
use prodex_domain::{SecretProvider as _, SecretPurpose, SecretResolutionRequest};

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
    use super::*;

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
}
