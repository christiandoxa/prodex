use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcPkceMethod {
    S256,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OidcBrowserFlowCapability {
    pub authorization_code: bool,
    pub pkce_s256: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OidcBrowserFlowRequirement {
    pub pkce_method: OidcPkceMethod,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OidcBrowserFlowCapabilityError {
    AuthorizationCodeUnsupported,
    PkceS256Unsupported,
}

impl fmt::Display for OidcBrowserFlowCapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OIDC browser flow capability is unavailable")
    }
}

impl Error for OidcBrowserFlowCapabilityError {}

pub fn require_oidc_browser_flow_capability(
    capability: OidcBrowserFlowCapability,
    requirement: OidcBrowserFlowRequirement,
) -> Result<(), OidcBrowserFlowCapabilityError> {
    if !capability.authorization_code {
        return Err(OidcBrowserFlowCapabilityError::AuthorizationCodeUnsupported);
    }
    if matches!(requirement.pkce_method, OidcPkceMethod::S256) && !capability.pkce_s256 {
        return Err(OidcBrowserFlowCapabilityError::PkceS256Unsupported);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_oidc_requires_explicit_authorization_code_and_pkce_s256_capabilities() {
        let requirement = OidcBrowserFlowRequirement {
            pkce_method: OidcPkceMethod::S256,
        };
        assert_eq!(
            require_oidc_browser_flow_capability(OidcBrowserFlowCapability::default(), requirement),
            Err(OidcBrowserFlowCapabilityError::AuthorizationCodeUnsupported)
        );
        assert_eq!(
            require_oidc_browser_flow_capability(
                OidcBrowserFlowCapability {
                    authorization_code: true,
                    pkce_s256: false,
                },
                requirement
            ),
            Err(OidcBrowserFlowCapabilityError::PkceS256Unsupported)
        );
        require_oidc_browser_flow_capability(
            OidcBrowserFlowCapability {
                authorization_code: true,
                pkce_s256: true,
            },
            requirement,
        )
        .unwrap();
    }
}
