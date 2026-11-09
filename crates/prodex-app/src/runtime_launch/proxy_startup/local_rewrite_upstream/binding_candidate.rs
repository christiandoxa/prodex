use super::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteBindingContext,
    RuntimeLocalRewriteProxyShared, runtime_local_rewrite_api_key_attempts,
};
use anyhow::Result;
use prodex_provider_core::ProviderId;
use prodex_provider_core::RuntimeProviderBindingIdentity;
use prodex_state::ResponseProfileBinding;

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

pub(super) fn runtime_local_rewrite_openai_attempts<'a>(
    shared: &RuntimeLocalRewriteProxyShared,
    api_keys: &'a [String],
    bound: Option<&ResponseProfileBinding>,
) -> Result<(Vec<(String, &'a str)>, bool)> {
    if let Some(bound) = bound {
        let Some(bound_identity) = bound.binding_identity.as_ref() else {
            return Err(anyhow::anyhow!(
                "OpenAI continuation binding has no exact key identity"
            ));
        };
        let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .filter(|(_, api_key)| {
                RuntimeProviderBindingIdentity::from_raw_key(
                    ProviderId::OpenAi,
                    api_key,
                    &shared.upstream_base_url,
                    None,
                )
                .is_some_and(|identity| identity == *bound_identity)
            })
            .collect::<Vec<_>>();
        if attempts.is_empty() {
            return Err(anyhow::anyhow!(
                "OpenAI continuation binding is unavailable or unauthorized"
            ));
        }
        return Ok((attempts, true));
    }
    let attempt_limit = shared
        .runtime_shared
        .runtime_config
        .tuning
        .precommit_attempt_limit
        .max(1);
    Ok((
        runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .take(attempt_limit)
            .collect(),
        false,
    ))
}
