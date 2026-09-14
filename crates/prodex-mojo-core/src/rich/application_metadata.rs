use super::*;

pub const APPLICATION_METADATA_ABI_VERSION: i64 = 1;
pub const APPLICATION_METADATA_MAX_HEADERS: usize = 64;
pub const APPLICATION_REQUEST_CONTEXT_ABI_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationCredentialScopePlan {
    DataPlane,
    ControlPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationRequestContextPlan {
    pub credential_scope: Option<ApplicationCredentialScopePlan>,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAuthorizationKind {
    DataPlane = 0,
    ControlPlane = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationAuthorizationDecision {
    AllowAnonymous,
    DataPlaneInference,
    DataPlaneQuota,
    ControlPlaneAction,
    WrongPlane,
    AnonymousNotAllowed,
    PrincipalMismatch,
}

/// Presence-only metadata derived from bounded header-name views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationRequestMetadataPlan {
    pub observed_header_count: usize,
    pub headers_truncated: bool,
    pub trace_context_present: bool,
    pub credential_present: bool,
    pub affinity_present: bool,
    pub codex_metadata_present: bool,
    pub user_agent_present: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationRequestMetadataResult {
    abi_version: i64,
    observed_header_count: i64,
    headers_truncated: i64,
    trace_context_present: i64,
    credential_present: i64,
    affinity_present: i64,
    codex_metadata_present: i64,
    user_agent_present: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationRequestContextPlanResult {
    abi_version: i64,
    credential_scope: i64,
}

const _: () = assert!(std::mem::size_of::<ApplicationRequestMetadataResult>() == 64);
const _: () = assert!(std::mem::size_of::<ApplicationRequestContextPlanResult>() == 16);

unsafe extern "C" {
    fn prodex_mojo_rich_application_request_context_plan_v1(
        abi_version: i64,
        route_kind: i64,
        route_plane: i64,
        result: u64,
    ) -> i64;

    fn prodex_mojo_rich_application_authorization_plan_v1(
        abi_version: i64,
        authorization_kind: i64,
        route_kind: i64,
        route_plane: i64,
        principal_present: i64,
        principal_matches_action: i64,
        decision: u64,
    ) -> i64;

    fn prodex_mojo_rich_application_request_metadata_v1(
        abi_version: i64,
        header_names: u64,
        header_count: i64,
        total_header_count: i64,
        result: u64,
    ) -> i64;
}

pub fn plan_application_authorization(
    authorization_kind: ApplicationAuthorizationKind,
    route_kind: i64,
    route_plane: i64,
    principal_present: bool,
    principal_matches_action: bool,
) -> Result<ApplicationAuthorizationDecision, MojoError> {
    ensure_rich_abi()?;
    let mut decision = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_application_authorization_plan_v1(
            APPLICATION_REQUEST_CONTEXT_ABI_VERSION,
            authorization_kind as i64,
            route_kind,
            route_plane,
            i64::from(principal_present),
            i64::from(principal_matches_action),
            mojo_mut_pointer_address(&mut decision),
        )
    };
    if status != 0 {
        return Err(status_error(status, 10, 0, 0, 0));
    }
    match decision {
        0 => Ok(ApplicationAuthorizationDecision::AllowAnonymous),
        1 => Ok(ApplicationAuthorizationDecision::DataPlaneInference),
        2 => Ok(ApplicationAuthorizationDecision::DataPlaneQuota),
        3 => Ok(ApplicationAuthorizationDecision::ControlPlaneAction),
        4 => Ok(ApplicationAuthorizationDecision::WrongPlane),
        5 => Ok(ApplicationAuthorizationDecision::AnonymousNotAllowed),
        6 => Ok(ApplicationAuthorizationDecision::PrincipalMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_application_request_context(
    route_kind: i64,
    route_plane: i64,
) -> Result<ApplicationRequestContextPlan, MojoError> {
    ensure_rich_abi()?;
    let mut result = ApplicationRequestContextPlanResult::default();
    let status = unsafe {
        prodex_mojo_rich_application_request_context_plan_v1(
            APPLICATION_REQUEST_CONTEXT_ABI_VERSION,
            route_kind,
            route_plane,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 10, 0, 0, 0));
    }
    if result.abi_version != APPLICATION_REQUEST_CONTEXT_ABI_VERSION {
        return Err(MojoError::InvalidOutput);
    }
    let credential_scope = match result.credential_scope {
        -1 => None,
        0 => Some(ApplicationCredentialScopePlan::DataPlane),
        1 => Some(ApplicationCredentialScopePlan::ControlPlane),
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(ApplicationRequestContextPlan { credential_scope })
}

/// Normalize header-name DTOs without crossing header values or credentials.
pub fn normalize_application_request_metadata(
    header_names: &[&str],
    total_header_count: usize,
) -> Result<ApplicationRequestMetadataPlan, MojoError> {
    ensure_rich_abi()?;
    if header_names.len() > APPLICATION_METADATA_MAX_HEADERS
        || total_header_count < header_names.len()
    {
        return Err(MojoError::InvalidInput);
    }
    let header_names = header_names
        .iter()
        .map(|name| view(name))
        .collect::<Vec<_>>();
    let mut result = ApplicationRequestMetadataResult::default();
    let status = unsafe {
        prodex_mojo_rich_application_request_metadata_v1(
            APPLICATION_METADATA_ABI_VERSION,
            mojo_pointer_address(header_names.as_ptr()),
            i64::try_from(header_names.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(total_header_count).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 10, 0, 0, 0));
    }
    if result.abi_version != APPLICATION_METADATA_ABI_VERSION
        || result.observed_header_count < 0
        || result.observed_header_count as usize > APPLICATION_METADATA_MAX_HEADERS
        || result.observed_header_count as usize != total_header_count.min(header_names.len())
        || result.headers_truncated != i64::from(total_header_count > header_names.len())
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationRequestMetadataPlan {
        observed_header_count: result.observed_header_count as usize,
        headers_truncated: decode_flag(result.headers_truncated)?,
        trace_context_present: decode_flag(result.trace_context_present)?,
        credential_present: decode_flag(result.credential_present)?,
        affinity_present: decode_flag(result.affinity_present)?,
        codex_metadata_present: decode_flag(result.codex_metadata_present)?,
        user_agent_present: decode_flag(result.user_agent_present)?,
    })
}

fn decode_flag(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
