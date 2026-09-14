use super::application_data_plane::{check_status, decode_bool};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationOperationalProbeMethod {
    Get,
    Head,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationOperationalProbeKind {
    Live,
    Ready,
    Startup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationOperationalProbeStatus {
    Ok,
    Draining,
    CredentialsStale,
    GovernanceAuditUnavailable,
    GovernancePolicyUnavailable,
    Overloaded,
    MethodNotAllowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationOperationalProbeMetric {
    Passing,
    Draining,
    Degraded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationOperationalProbeInput {
    pub route: ApplicationRouteKind,
    pub method: ApplicationOperationalProbeMethod,
    pub overloaded: bool,
    pub draining: bool,
    pub credentials_stale: bool,
    pub governance_audit_available: bool,
    pub governance_policy_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationOperationalProbePlan {
    pub method_allowed: bool,
    pub probe: Option<ApplicationOperationalProbeKind>,
    pub ready: bool,
    pub status: ApplicationOperationalProbeStatus,
    pub metric: ApplicationOperationalProbeMetric,
    pub http_status: u16,
    pub omit_body: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationOperationalProbePlanResult {
    abi_version: i64,
    method_allowed: i64,
    probe_kind: i64,
    ready: i64,
    status_reason: i64,
    metric_result: i64,
    http_status: i64,
    omit_body: i64,
}

const _: () = assert!(std::mem::size_of::<ApplicationOperationalProbePlanResult>() == 64);

unsafe extern "C" {
    fn prodex_mojo_application_operational_probe_plan_v1(
        abi_version: i64,
        route: i64,
        method: i64,
        overloaded: i64,
        draining: i64,
        credentials_stale: i64,
        governance_audit_available: i64,
        governance_policy_available: i64,
        result: u64,
    ) -> i64;
}

pub fn plan_application_operational_probe(
    input: ApplicationOperationalProbeInput,
) -> Result<ApplicationOperationalProbePlan, MojoError> {
    ensure_rich_abi()?;
    let mut result = ApplicationOperationalProbePlanResult::default();
    let status = unsafe {
        prodex_mojo_application_operational_probe_plan_v1(
            APPLICATION_DATA_PLANE_ABI_VERSION,
            input.route as i64,
            match input.method {
                ApplicationOperationalProbeMethod::Get => 0,
                ApplicationOperationalProbeMethod::Head => 1,
                ApplicationOperationalProbeMethod::Other => 2,
            },
            i64::from(input.overloaded),
            i64::from(input.draining),
            i64::from(input.credentials_stale),
            i64::from(input.governance_audit_available),
            i64::from(input.governance_policy_available),
            mojo_mut_pointer_address(&mut result),
        )
    };
    check_status(status)?;
    if result.abi_version != APPLICATION_DATA_PLANE_ABI_VERSION {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationOperationalProbePlan {
        method_allowed: decode_bool(result.method_allowed)?,
        probe: match result.probe_kind {
            -1 => None,
            0 => Some(ApplicationOperationalProbeKind::Live),
            1 => Some(ApplicationOperationalProbeKind::Ready),
            2 => Some(ApplicationOperationalProbeKind::Startup),
            _ => return Err(MojoError::InvalidOutput),
        },
        ready: decode_bool(result.ready)?,
        status: match result.status_reason {
            0 => ApplicationOperationalProbeStatus::Ok,
            1 => ApplicationOperationalProbeStatus::Draining,
            2 => ApplicationOperationalProbeStatus::CredentialsStale,
            3 => ApplicationOperationalProbeStatus::GovernanceAuditUnavailable,
            4 => ApplicationOperationalProbeStatus::GovernancePolicyUnavailable,
            5 => ApplicationOperationalProbeStatus::Overloaded,
            6 => ApplicationOperationalProbeStatus::MethodNotAllowed,
            _ => return Err(MojoError::InvalidOutput),
        },
        metric: match result.metric_result {
            0 => ApplicationOperationalProbeMetric::Passing,
            1 => ApplicationOperationalProbeMetric::Draining,
            2 => ApplicationOperationalProbeMetric::Degraded,
            _ => return Err(MojoError::InvalidOutput),
        },
        http_status: u16::try_from(result.http_status).map_err(|_| MojoError::InvalidOutput)?,
        omit_body: decode_bool(result.omit_body)?,
    })
}
