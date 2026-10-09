use super::{MojoError, ensure_rich_abi, mojo_mut_pointer_address, view};

const PROVIDER_BINDING_IDENTITY_ABI_VERSION: i64 = 1;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBindingIdentityInputPlan {
    pub credential: String,
    pub endpoint: String,
    pub profile: Option<String>,
}

unsafe extern "C" {
    fn prodex_mojo_provider_binding_identity_plan_v1(
        abi_version: i64,
        credential_address: u64,
        credential_length: i64,
        endpoint_address: u64,
        endpoint_length: i64,
        profile_address: u64,
        profile_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_provider_binding_identity_digest_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn optional_view(value: Option<&str>) -> Result<(u64, i64), MojoError> {
    value
        .map(|value| Ok((view(value).ptr, signed_len(value)?)))
        .unwrap_or(Ok((0, 0)))
}

fn bounded_slice(value: &str, start: i64, end: i64) -> Result<&str, MojoError> {
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > value.len() {
        return Err(MojoError::InvalidOutput);
    }
    value.get(start..end).ok_or(MojoError::InvalidOutput)
}

pub fn provider_binding_identity_inputs(
    credential: &str,
    endpoint: &str,
    profile: Option<&str>,
) -> Result<Option<ProviderBindingIdentityInputPlan>, MojoError> {
    ensure_rich_abi()?;
    let (profile_address, profile_length) = optional_view(profile)?;
    let mut output = [0_i64; 6];
    let status = unsafe {
        prodex_mojo_provider_binding_identity_plan_v1(
            PROVIDER_BINDING_IDENTITY_ABI_VERSION,
            view(credential).ptr,
            signed_len(credential)?,
            view(endpoint).ptr,
            signed_len(endpoint)?,
            profile_address,
            profile_length,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    match status {
        0 => {
            let credential = bounded_slice(credential, output[0], output[1])?;
            let endpoint = bounded_slice(endpoint, output[2], output[3])?;
            let profile = bounded_slice(profile.unwrap_or_default(), output[4], output[5])?;
            Ok(Some(ProviderBindingIdentityInputPlan {
                credential: credential.to_string(),
                endpoint: endpoint.to_string(),
                profile: (!profile.is_empty()).then(|| profile.to_string()),
            }))
        }
        1 => Ok(None),
        2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_binding_identity_digest_is_public(value: &str) -> Result<bool, MojoError> {
    if value.len() > 71 {
        return Ok(false);
    }
    ensure_rich_abi()?;
    match unsafe {
        prodex_mojo_provider_binding_identity_digest_valid_v1(
            PROVIDER_BINDING_IDENTITY_ABI_VERSION,
            view(value).ptr,
            signed_len(value)?,
        )
    } {
        0 => Ok(true),
        1 => Ok(false),
        2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::{provider_binding_identity_digest_is_public, provider_binding_identity_inputs};

    #[test]
    fn input_plan_trims_unicode_and_removes_endpoint_slashes() {
        let plan = provider_binding_identity_inputs(
            "\u{2003}credential-🦀\u{3000}",
            "https://example.com/v1///",
            Some("\u{2003}profile-雪\u{3000}"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.credential, "credential-🦀");
        assert_eq!(plan.endpoint, "https://example.com/v1");
        assert_eq!(plan.profile.as_deref(), Some("profile-雪"));
    }

    #[test]
    fn input_plan_preserves_empty_profile_and_exact_byte_limits() {
        assert_eq!(
            provider_binding_identity_inputs(
                "credential",
                "https://example.com",
                Some(" \u{2003}")
            )
            .unwrap()
            .unwrap()
            .profile,
            None
        );
        assert!(
            provider_binding_identity_inputs(&"x".repeat(4_096), "https://example.com", None,)
                .unwrap()
                .is_some()
        );
        assert!(
            provider_binding_identity_inputs(&"x".repeat(4_097), "https://example.com", None,)
                .unwrap()
                .is_none()
        );
        assert!(
            provider_binding_identity_inputs(
                "credential",
                "https://example.com",
                Some(&"x".repeat(256)),
            )
            .unwrap()
            .is_some()
        );
        assert!(
            provider_binding_identity_inputs(
                "credential",
                "https://example.com",
                Some(&"x".repeat(257)),
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn input_plan_rejects_controls_and_digest_validation_is_exact() {
        assert!(
            provider_binding_identity_inputs("credential\u{0000}", "https://example.com", None,)
                .unwrap()
                .is_none()
        );
        let valid = format!("sha256:{}", "a".repeat(64));
        assert!(provider_binding_identity_digest_is_public(&valid).unwrap());
        assert!(!provider_binding_identity_digest_is_public(&valid.to_uppercase()).unwrap());
        assert!(!provider_binding_identity_digest_is_public("sha256:abc").unwrap());
        assert!(
            !provider_binding_identity_digest_is_public(&format!("sha256:{}x", "a".repeat(64)))
                .unwrap()
        );
    }

    #[test]
    fn input_plan_matches_legacy_normalization_for_unicode_and_oversized_padding() {
        fn legacy(value: &str, maximum: usize) -> Option<String> {
            let trimmed = value.trim();
            (!trimmed.is_empty()
                && trimmed.len() <= maximum
                && trimmed.chars().all(|character| !character.is_control()))
            .then(|| trimmed.to_string())
        }

        let padded = format!(
            "{}credential{}",
            "\u{2003}".repeat(40_000),
            "\u{3000}".repeat(40_000)
        );
        for credential in [
            "",
            " \u{2003}",
            "\u{2003}credential-雪\u{3000}",
            "credential\u{0085}control",
            "credential\u{007f}control",
            &"x".repeat(4_096),
            &"x".repeat(4_097),
            padded.as_str(),
        ] {
            let expected_credential = legacy(credential, 4_096);
            let actual = provider_binding_identity_inputs(
                credential,
                "https://example.com/v1///",
                Some("\u{2003}profile-雪\u{3000}"),
            )
            .unwrap();
            assert_eq!(
                actual.as_ref().map(|plan| plan.credential.as_str()),
                expected_credential.as_deref(),
                "credential={credential:?}"
            );
            if let (Some(actual), Some(expected)) = (actual, expected_credential) {
                assert_eq!(actual.credential, expected);
                assert_eq!(actual.endpoint, "https://example.com/v1");
                assert_eq!(actual.profile.as_deref(), Some("profile-雪"));
            }
        }
    }
}
