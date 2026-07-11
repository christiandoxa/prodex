use super::super::*;
use prodex_authn::{OidcEndpointPolicy, ValidatedOidcEndpoint};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
#[cfg(test)]
use std::net::IpAddr;

#[derive(Clone)]
pub(super) enum RuntimeGatewayOidcEndpoint {
    Validated(ValidatedOidcEndpoint),
    #[cfg(test)]
    InsecureLoopback(String),
}

impl RuntimeGatewayOidcEndpoint {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Validated(endpoint) => endpoint.as_str(),
            #[cfg(test)]
            Self::InsecureLoopback(endpoint) => endpoint,
        }
    }

    pub(super) fn cache_key(&self) -> String {
        let digest = Sha256::digest(self.as_str().as_bytes());
        let mut key = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(key, "{byte:02x}").expect("writing to a String cannot fail");
        }
        key
    }
}

pub(super) enum RuntimeGatewayOidcEndpointPolicy {
    Validated(Box<OidcEndpointPolicy>),
    #[cfg(test)]
    InsecureLoopback {
        issuer: String,
        configured_jwks: Option<String>,
    },
}

impl RuntimeGatewayOidcEndpointPolicy {
    pub(super) fn from_config(config: &RuntimeGatewayOidcConfig) -> Result<Self> {
        match OidcEndpointPolicy::with_jwks_origin_allowlist(
            &config.issuer,
            config.jwks_url.as_deref(),
            config.jwks_origin_allowlist.iter().map(String::as_str),
        ) {
            Ok(policy) => Ok(Self::Validated(Box::new(policy))),
            Err(error) => {
                #[cfg(test)]
                if runtime_gateway_oidc_test_policy_is_allowed(config) {
                    return Ok(Self::InsecureLoopback {
                        issuer: config.issuer.clone(),
                        configured_jwks: config.jwks_url.clone(),
                    });
                }
                Err(error).context("gateway OIDC endpoint policy is invalid")
            }
        }
    }

    pub(super) fn configured_jwks(&self) -> Option<RuntimeGatewayOidcEndpoint> {
        match self {
            Self::Validated(policy) => policy
                .configured_jwks()
                .cloned()
                .map(RuntimeGatewayOidcEndpoint::Validated),
            #[cfg(test)]
            Self::InsecureLoopback {
                configured_jwks, ..
            } => configured_jwks
                .clone()
                .map(RuntimeGatewayOidcEndpoint::InsecureLoopback),
        }
    }

    pub(super) fn discovery_endpoint(&self) -> RuntimeGatewayOidcEndpoint {
        match self {
            Self::Validated(policy) => {
                RuntimeGatewayOidcEndpoint::Validated(policy.discovery_endpoint())
            }
            #[cfg(test)]
            Self::InsecureLoopback { issuer, .. } => {
                RuntimeGatewayOidcEndpoint::InsecureLoopback(format!(
                    "{}/.well-known/openid-configuration",
                    issuer.trim_end_matches('/')
                ))
            }
        }
    }

    pub(super) fn validate_discovery_document(
        &self,
        document: &serde_json::Value,
    ) -> Result<RuntimeGatewayOidcEndpoint> {
        let document_issuer = document
            .get("issuer")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("gateway OIDC discovery document is missing issuer"))?;
        let jwks_uri = document
            .get("jwks_uri")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                anyhow::anyhow!("gateway OIDC discovery document is missing jwks_uri")
            })?;
        match self {
            Self::Validated(policy) => policy
                .validate_discovery_document(document_issuer, jwks_uri)
                .map(RuntimeGatewayOidcEndpoint::Validated)
                .context("gateway OIDC discovery document is not permitted"),
            #[cfg(test)]
            Self::InsecureLoopback { issuer, .. } => {
                if document_issuer.trim_end_matches('/') != issuer.trim_end_matches('/') {
                    bail!("test OIDC discovery issuer mismatch");
                }
                runtime_gateway_oidc_test_endpoint(jwks_uri)
                    .map(RuntimeGatewayOidcEndpoint::InsecureLoopback)
            }
        }
    }
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_test_policy_is_allowed(
    config: &RuntimeGatewayOidcConfig,
) -> bool {
    let issuer_is_valid = reqwest::Url::parse(&config.issuer).is_ok_and(|issuer| {
        (issuer.scheme() == "https"
            && issuer.host_str().is_some()
            && issuer.username().is_empty()
            && issuer.password().is_none()
            && issuer.query().is_none()
            && issuer.fragment().is_none())
            || runtime_gateway_oidc_test_url_is_insecure_loopback(&issuer)
    });
    issuer_is_valid
        && config.jwks_origin_allowlist.is_empty()
        && config.jwks_url.as_deref().is_none_or(|endpoint| {
            reqwest::Url::parse(endpoint)
                .is_ok_and(|endpoint| runtime_gateway_oidc_test_url_is_insecure_loopback(&endpoint))
        })
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_test_endpoint(value: &str) -> Result<String> {
    let endpoint = reqwest::Url::parse(value).context("test OIDC endpoint is invalid")?;
    if !runtime_gateway_oidc_test_url_is_insecure_loopback(&endpoint) {
        bail!("test OIDC endpoint must use insecure loopback");
    }
    Ok(endpoint.to_string())
}

#[cfg(test)]
pub(super) fn runtime_gateway_oidc_test_url_is_insecure_loopback(url: &reqwest::Url) -> bool {
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url
            .host_str()
            .and_then(|host| host.parse::<IpAddr>().ok())
            .is_some_and(|address| address.is_loopback())
}
