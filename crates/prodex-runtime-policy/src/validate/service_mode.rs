use crate::types::{RuntimePolicyFile, RuntimePolicyServiceMode};
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn validate_service_mode(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if policy.service_mode == RuntimePolicyServiceMode::Gateway {
        return Ok(());
    }
    if !policy.secrets.production {
        bail!(
            "secrets.production in {} must be true when service_mode=control-plane",
            path.display()
        );
    }
    if let Some(field) = control_plane_data_plane_field(policy) {
        bail!(
            "{field} in {} is forbidden when service_mode=control-plane",
            path.display()
        );
    }
    let has_projected_admin = policy.gateway.admin_tokens.iter().any(|token| {
        token.token_ref.is_some()
            && token.token_env.is_empty()
            && token.role.as_deref() == Some("admin")
    });
    if !has_projected_admin {
        bail!(
            "gateway.admin_tokens in {} must include a projected admin-role token when service_mode=control-plane",
            path.display()
        );
    }
    let state = &policy.gateway.state;
    let shared_state_configured = match state.backend.as_deref().map(str::to_ascii_lowercase) {
        Some(backend) if backend == "postgres" => state.postgres_url_ref.is_some(),
        Some(backend) if backend == "redis" => state.redis_url_ref.is_some(),
        _ => false,
    };
    if !shared_state_configured {
        bail!(
            "gateway.state in {} must configure postgres or redis with a projected URL reference when service_mode=control-plane",
            path.display()
        );
    }
    Ok(())
}

fn control_plane_data_plane_field(policy: &RuntimePolicyFile) -> Option<&'static str> {
    let gateway = &policy.gateway;
    [
        ("gateway.provider", gateway.provider.is_some()),
        ("gateway.base_url", gateway.base_url.is_some()),
        ("gateway.require_auth", gateway.require_auth.is_some()),
        ("gateway.auth_token_ref", gateway.auth_token_ref.is_some()),
        (
            "gateway.provider_api_key_ref",
            gateway.provider_api_key_ref.is_some(),
        ),
        (
            "gateway.adaptive_routing",
            gateway.adaptive_routing != Default::default(),
        ),
        ("gateway.route_aliases", !gateway.route_aliases.is_empty()),
        (
            "gateway.request_constraints",
            gateway.request_constraints != Default::default(),
        ),
        ("gateway.virtual_keys", !gateway.virtual_keys.is_empty()),
        (
            "gateway.observability",
            gateway.observability != Default::default(),
        ),
        (
            "gateway.guardrails",
            gateway.guardrails != Default::default(),
        ),
    ]
    .into_iter()
    .find_map(|(field, configured)| configured.then_some(field))
}
