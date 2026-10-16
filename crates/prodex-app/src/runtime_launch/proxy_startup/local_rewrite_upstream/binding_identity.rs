use super::super::local_rewrite::RuntimeLocalRewriteProxyShared;
use prodex_provider_core::{ProviderId, RuntimeProviderBindingIdentity};

pub(super) fn runtime_local_rewrite_identity_candidates(
    provider: ProviderId,
    credential: Option<&str>,
    endpoint: &str,
    profile: Option<&str>,
) -> (
    Option<RuntimeProviderBindingIdentity>,
    Option<RuntimeProviderBindingIdentity>,
) {
    let credential_identity = credential.and_then(|credential| {
        RuntimeProviderBindingIdentity::from_raw_key(provider, credential, endpoint, profile)
    });
    let profile_identity = (credential_identity.is_none())
        .then(|| {
            profile.and_then(|profile| {
                RuntimeProviderBindingIdentity::from_profile(provider, profile, endpoint)
            })
        })
        .flatten();
    (credential_identity, profile_identity)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_raw_binding_identity(
    _shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
    credential: Option<&str>,
    endpoint: &str,
    profile: Option<&str>,
) -> Option<RuntimeProviderBindingIdentity> {
    let (credential_identity, profile_identity) =
        runtime_local_rewrite_identity_candidates(provider, credential, endpoint, profile);
    match prodex_mojo_core::runtime_lineage::binding_identity_source(
        credential.is_some(),
        credential_identity.is_some(),
        profile.is_some(),
        profile_identity.is_some(),
    )
    .expect("Mojo binding-identity source policy returned invalid output")
    {
        prodex_mojo_core::runtime_lineage::RuntimeLineageBindingIdentitySource::None => None,
        prodex_mojo_core::runtime_lineage::RuntimeLineageBindingIdentitySource::Credential => {
            credential_identity
        }
        prodex_mojo_core::runtime_lineage::RuntimeLineageBindingIdentitySource::Profile => {
            profile_identity
        }
    }
}
