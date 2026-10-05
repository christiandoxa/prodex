use super::{ABI_VERSION, signed_len};
use crate::MojoError;

const LOCAL_REWRITE: i64 = 0;
const DISPATCH: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_lineage_binding_candidate_allowed_v1(
        abi_version: i64,
        mode: i64,
        expected_profile_address: u64,
        expected_profile_length: i64,
        bound_profile_present: i64,
        bound_profile_address: u64,
        bound_profile_length: i64,
        bound_identity_present: i64,
        bound_provider: i64,
        bound_credential_address: u64,
        bound_credential_length: i64,
        bound_endpoint_address: u64,
        bound_endpoint_length: i64,
        bound_profile_identity_present: i64,
        bound_profile_identity_address: u64,
        bound_profile_identity_length: i64,
        candidate_provider_present: i64,
        candidate_provider: i64,
        candidate_identity_present: i64,
        candidate_identity_provider: i64,
        candidate_credential_address: u64,
        candidate_credential_length: i64,
        candidate_endpoint_address: u64,
        candidate_endpoint_length: i64,
        candidate_profile_identity_present: i64,
        candidate_profile_identity_address: u64,
        candidate_profile_identity_length: i64,
    ) -> i64;
}

/// Borrowed, secret-free provider identity fields used for hard-binding decisions.
///
/// `provider_id` uses `ProviderId`'s `repr(i64)` value. Credential, endpoint, and optional
/// profile identities must be the precomputed public identity digests; raw credentials never
/// cross the Mojo boundary.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeLineageProviderBindingIdentity<'a> {
    /// Numeric `ProviderId` value.
    pub provider_id: i64,
    /// Rust-computed credential identity digest.
    pub credential_identity: &'a str,
    /// Rust-computed endpoint identity digest.
    pub endpoint_identity: &'a str,
    /// Optional Rust-computed profile identity digest.
    pub profile_identity: Option<&'a str>,
}

/// Inputs for local rewrite credential eligibility under a persisted hard binding.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeLineageLocalRewriteCandidate<'a> {
    /// Expected local rewrite profile name.
    pub expected_profile: &'a str,
    /// Persisted binding profile name, or `None` when request has no binding.
    pub bound_profile: Option<&'a str>,
    /// Persisted secret-free provider identity, when present.
    pub bound_identity: Option<RuntimeLineageProviderBindingIdentity<'a>>,
    /// Candidate secret-free provider identity, when available.
    pub candidate_identity: Option<RuntimeLineageProviderBindingIdentity<'a>>,
}

/// Inputs for dispatch-time hard-binding validation.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeLineageDispatchBindingCandidate<'a> {
    /// Selected provider's numeric `ProviderId` value.
    pub selected_provider_id: i64,
    /// Persisted secret-free provider identity, when present.
    pub bound_identity: Option<RuntimeLineageProviderBindingIdentity<'a>>,
    /// Selected identity, when dispatch has resolved it.
    pub selected_identity: Option<RuntimeLineageProviderBindingIdentity<'a>>,
}

/// Dispatch-time hard-binding outcome returned by Mojo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageDispatchBindingDecision {
    /// Binding permits the selected provider and identity.
    Allowed,
    /// Bound provider differs from the selected provider.
    ProviderUnavailable,
    /// Resolved provider identity differs from the bound identity.
    IdentityUnavailable,
}

fn text_parts(value: Option<&str>) -> Result<(i64, u64, i64), MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0));
    };
    Ok((1, value.as_ptr() as usize as u64, signed_len(value)?))
}

#[derive(Clone, Copy, Default)]
struct BindingIdentityParts {
    present: i64,
    provider_id: i64,
    credential_address: u64,
    credential_length: i64,
    endpoint_address: u64,
    endpoint_length: i64,
    profile_present: i64,
    profile_address: u64,
    profile_length: i64,
}

fn binding_identity_parts(
    identity: Option<RuntimeLineageProviderBindingIdentity<'_>>,
) -> Result<BindingIdentityParts, MojoError> {
    let Some(identity) = identity else {
        return Ok(BindingIdentityParts::default());
    };
    let (profile_present, profile_address, profile_length) = text_parts(identity.profile_identity)?;
    Ok(BindingIdentityParts {
        present: 1,
        provider_id: identity.provider_id,
        credential_address: identity.credential_identity.as_ptr() as usize as u64,
        credential_length: signed_len(identity.credential_identity)?,
        endpoint_address: identity.endpoint_identity.as_ptr() as usize as u64,
        endpoint_length: signed_len(identity.endpoint_identity)?,
        profile_present,
        profile_address,
        profile_length,
    })
}

fn binding_candidate_result(
    mode: i64,
    expected_profile: Option<&str>,
    bound_profile: Option<&str>,
    bound_identity: Option<RuntimeLineageProviderBindingIdentity<'_>>,
    candidate_provider_id: Option<i64>,
    candidate_identity: Option<RuntimeLineageProviderBindingIdentity<'_>>,
) -> Result<i64, MojoError> {
    let (expected_present, expected_address, expected_length) = text_parts(expected_profile)?;
    let (bound_profile_present, bound_profile_address, bound_profile_length) =
        text_parts(bound_profile)?;
    let bound = binding_identity_parts(bound_identity)?;
    let candidate = binding_identity_parts(candidate_identity)?;
    match unsafe {
        prodex_runtime_lineage_binding_candidate_allowed_v1(
            ABI_VERSION,
            mode,
            if expected_present == 1 {
                expected_address
            } else {
                0
            },
            expected_length,
            bound_profile_present,
            bound_profile_address,
            bound_profile_length,
            bound.present,
            bound.provider_id,
            bound.credential_address,
            bound.credential_length,
            bound.endpoint_address,
            bound.endpoint_length,
            bound.profile_present,
            bound.profile_address,
            bound.profile_length,
            i64::from(candidate_provider_id.is_some()),
            candidate_provider_id.unwrap_or_default(),
            candidate.present,
            candidate.provider_id,
            candidate.credential_address,
            candidate.credential_length,
            candidate.endpoint_address,
            candidate.endpoint_length,
            candidate.profile_present,
            candidate.profile_address,
            candidate.profile_length,
        )
    } {
        result @ 0..=2 => Ok(result),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Decide whether a provider credential can serve a local rewrite request.
///
/// An unbound request accepts any available candidate. A bound request requires the expected
/// profile and every secret-free provider identity field to match exactly.
pub fn local_rewrite_candidate_allowed(
    input: RuntimeLineageLocalRewriteCandidate<'_>,
) -> Result<bool, MojoError> {
    match binding_candidate_result(
        LOCAL_REWRITE,
        Some(input.expected_profile),
        input.bound_profile,
        input.bound_identity,
        input
            .candidate_identity
            .map(|identity| identity.provider_id),
        input.candidate_identity,
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Validate the selected provider and optional resolved identity against a hard binding.
///
/// When the binding has no identity, or dispatch has not resolved one yet, retain the legacy
/// provider-only validation behavior.
pub fn dispatch_binding_candidate_decision(
    input: RuntimeLineageDispatchBindingCandidate<'_>,
) -> Result<RuntimeLineageDispatchBindingDecision, MojoError> {
    match binding_candidate_result(
        DISPATCH,
        None,
        None,
        input.bound_identity,
        Some(input.selected_provider_id),
        input.selected_identity,
    )? {
        0 => Ok(RuntimeLineageDispatchBindingDecision::Allowed),
        1 => Ok(RuntimeLineageDispatchBindingDecision::ProviderUnavailable),
        2 => Ok(RuntimeLineageDispatchBindingDecision::IdentityUnavailable),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod binding_candidate_tests {
    use super::*;

    fn identity(
        provider_id: i64,
        credential_identity: &'static str,
        endpoint_identity: &'static str,
        profile_identity: Option<&'static str>,
    ) -> RuntimeLineageProviderBindingIdentity<'static> {
        RuntimeLineageProviderBindingIdentity {
            provider_id,
            credential_identity,
            endpoint_identity,
            profile_identity,
        }
    }

    #[test]
    fn local_rewrite_candidate_requires_exact_profile_and_identity() {
        let bound = identity(
            1,
            "sha256:credential-a",
            "sha256:endpoint-a",
            Some("sha256:profile-a"),
        );
        let allowed = |bound_profile, bound_identity, candidate_identity| {
            local_rewrite_candidate_allowed(RuntimeLineageLocalRewriteCandidate {
                expected_profile: "local",
                bound_profile,
                bound_identity,
                candidate_identity,
            })
            .unwrap()
        };

        assert!(allowed(None, None, Some(bound)));
        assert!(!allowed(None, None, None));
        assert!(allowed(Some("local"), Some(bound), Some(bound)));
        assert!(!allowed(Some("other"), Some(bound), Some(bound)));
        assert!(!allowed(Some("local"), None, Some(bound)));
        assert!(!allowed(
            Some("local"),
            Some(bound),
            Some(identity(
                1,
                "sha256:credential-b",
                "sha256:endpoint-a",
                Some("sha256:profile-a"),
            )),
        ));
        assert!(!allowed(
            Some("local"),
            Some(bound),
            Some(identity(
                1,
                "sha256:credential-a",
                "sha256:endpoint-b",
                Some("sha256:profile-a"),
            )),
        ));
        assert!(!allowed(
            Some("local"),
            Some(bound),
            Some(identity(
                1,
                "sha256:credential-a",
                "sha256:endpoint-a",
                None,
            )),
        ));
        assert!(!allowed(
            Some("local"),
            Some(bound),
            Some(identity(
                2,
                "sha256:credential-a",
                "sha256:endpoint-a",
                Some("sha256:profile-a"),
            )),
        ));
    }

    #[test]
    fn dispatch_binding_preserves_provider_only_and_exact_identity_rules() {
        let bound = identity(
            1,
            "sha256:credential-a",
            "sha256:endpoint-a",
            Some("sha256:profile-a"),
        );
        let decision = |selected_provider_id, bound_identity, selected_identity| {
            dispatch_binding_candidate_decision(RuntimeLineageDispatchBindingCandidate {
                selected_provider_id,
                bound_identity,
                selected_identity,
            })
            .unwrap()
        };

        assert_eq!(
            decision(1, None, None),
            RuntimeLineageDispatchBindingDecision::Allowed
        );
        assert_eq!(
            decision(1, Some(bound), None),
            RuntimeLineageDispatchBindingDecision::Allowed
        );
        assert_eq!(
            decision(1, Some(bound), Some(bound)),
            RuntimeLineageDispatchBindingDecision::Allowed
        );
        assert_eq!(
            decision(2, Some(bound), None),
            RuntimeLineageDispatchBindingDecision::ProviderUnavailable
        );
        assert_eq!(
            decision(
                1,
                Some(bound),
                Some(identity(
                    1,
                    "sha256:credential-b",
                    "sha256:endpoint-a",
                    Some("sha256:profile-a"),
                )),
            ),
            RuntimeLineageDispatchBindingDecision::IdentityUnavailable
        );
    }
}
