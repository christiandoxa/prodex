mod admin;
mod cache;
mod endpoint_policy;
mod token_claims;
mod transport;

pub(super) use admin::{
    RuntimeGatewayAdminAuth, RuntimeGatewayAdminAuthentication,
    RuntimeGatewayAdminCredentialEvidence, RuntimeGatewayOidcAdminCredentialEvidence,
    runtime_gateway_admin_auth, runtime_gateway_admin_auth_is_unscoped,
    runtime_gateway_admin_auth_matches_entry, runtime_gateway_oidc_admin_auth_from_verified,
};
pub(super) use cache::{
    RuntimeGatewayOidcJwksSnapshot, runtime_gateway_prefetch_oidc_cache,
    runtime_gateway_run_oidc_background_refresh_loop,
};
pub(super) use endpoint_policy::runtime_gateway_oidc_browser_endpoint;
#[cfg(test)]
pub(super) use token_claims::runtime_gateway_test_verified_oidc_token;
pub(super) use token_claims::{
    RuntimeGatewayVerifiedOidcToken, runtime_gateway_verify_oidc_logout_token,
    runtime_gateway_verify_oidc_token, runtime_gateway_verify_workload_token,
};
pub(super) use transport::runtime_gateway_oidc_post_form;

#[cfg(test)]
mod tests;
