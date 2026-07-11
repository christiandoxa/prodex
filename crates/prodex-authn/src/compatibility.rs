use std::error::Error;
use std::fmt;

use prodex_domain::{CredentialScope, Principal};

/// Authentication result supplied by a compatibility transport adapter.
///
/// The adapter still verifies the credential secret. This boundary owns the
/// fail-closed route-scope binding while that verification is migrated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityAuthenticationRequest {
    pub principal: Option<Principal>,
    pub required_scope: Option<CredentialScope>,
    pub anonymous_allowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibilityAuthenticationError {
    CredentialRequired,
    CredentialScopeMismatch,
}

impl fmt::Display for CompatibilityAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("request authentication is denied")
    }
}

impl Error for CompatibilityAuthenticationError {}

pub fn authenticate_compatibility_request(
    request: CompatibilityAuthenticationRequest,
) -> Result<Option<Principal>, CompatibilityAuthenticationError> {
    match (request.principal, request.required_scope) {
        (Some(principal), Some(required)) if principal.credential_scope != required => {
            Err(CompatibilityAuthenticationError::CredentialScopeMismatch)
        }
        (Some(principal), _) => Ok(Some(principal)),
        (None, Some(_)) if !request.anonymous_allowed => {
            Err(CompatibilityAuthenticationError::CredentialRequired)
        }
        (None, _) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{PrincipalId, PrincipalKind, Role};

    fn principal(scope: CredentialScope) -> Principal {
        Principal::new(
            "00000000-0000-7000-8000-000000000001"
                .parse::<PrincipalId>()
                .unwrap(),
            None,
            PrincipalKind::ServiceAccount,
            Role::Operator,
            scope,
        )
    }

    #[test]
    fn compatibility_authentication_preserves_legacy_open_mode_but_binds_scopes() {
        let cases = [
            (None, Some(CredentialScope::DataPlane), true, Ok(None)),
            (
                None,
                Some(CredentialScope::DataPlane),
                false,
                Err(CompatibilityAuthenticationError::CredentialRequired),
            ),
            (
                Some(principal(CredentialScope::DataPlane)),
                Some(CredentialScope::DataPlane),
                false,
                Ok(Some(principal(CredentialScope::DataPlane))),
            ),
            (
                Some(principal(CredentialScope::DataPlane)),
                Some(CredentialScope::ControlPlane),
                false,
                Err(CompatibilityAuthenticationError::CredentialScopeMismatch),
            ),
        ];

        for (principal, required_scope, anonymous_allowed, expected) in cases {
            assert_eq!(
                authenticate_compatibility_request(CompatibilityAuthenticationRequest {
                    principal,
                    required_scope,
                    anonymous_allowed,
                }),
                expected,
            );
        }
    }
}
