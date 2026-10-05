use super::{RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteBindingContext};
use prodex_provider_core::RuntimeProviderBindingIdentity;

pub(super) fn candidate_allowed(
    binding: &RuntimeLocalRewriteBindingContext,
    identity: Option<&RuntimeProviderBindingIdentity>,
) -> bool {
    prodex_mojo_core::runtime_lineage::local_rewrite_candidate_allowed(
        prodex_mojo_core::runtime_lineage::RuntimeLineageLocalRewriteCandidate {
            expected_profile: RUNTIME_LOCAL_REWRITE_PROFILE,
            bound_profile: binding
                .bound
                .as_ref()
                .map(|bound| bound.profile_name.as_str()),
            bound_identity: binding
                .bound
                .as_ref()
                .and_then(|bound| bound.binding_identity.as_ref())
                .map(runtime_local_rewrite_mojo_binding_identity),
            candidate_identity: identity.map(runtime_local_rewrite_mojo_binding_identity),
        },
    )
    .expect("Mojo local rewrite binding-candidate policy returned invalid output")
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_mojo_binding_identity(
    identity: &RuntimeProviderBindingIdentity,
) -> prodex_mojo_core::runtime_lineage::RuntimeLineageProviderBindingIdentity<'_> {
    prodex_mojo_core::runtime_lineage::RuntimeLineageProviderBindingIdentity {
        provider_id: identity.provider() as i64,
        credential_identity: identity.credential_identity(),
        endpoint_identity: identity.endpoint_identity(),
        profile_identity: identity.profile(),
    }
}
