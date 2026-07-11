use super::*;
use crate::route::path_without_query_or_fragment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayHttpApiVersionPlan {
    pub requested: ApiVersion,
    pub decision: ApiVersionDecision,
    pub explicit_path_version: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpApiVersionErrorStatus {
    NotFound,
    Gone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpApiVersionErrorResponsePlan {
    pub status: GatewayHttpApiVersionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_api_version(
    path: &str,
    default_version: ApiVersion,
    policies: &[prodex_domain::ApiVersionPolicy],
    now_unix_ms: u64,
) -> Result<GatewayHttpApiVersionPlan, ApiVersionError> {
    let (requested, explicit_path_version) = requested_api_version_for_path(path, default_version);
    let decision = evaluate_api_version(requested, policies, now_unix_ms)?;
    Ok(GatewayHttpApiVersionPlan {
        requested,
        decision,
        explicit_path_version,
    })
}

pub fn plan_gateway_http_api_version_error_response(
    error: &ApiVersionError,
) -> GatewayHttpApiVersionErrorResponsePlan {
    let response = plan_api_version_error_response(error);
    GatewayHttpApiVersionErrorResponsePlan {
        status: match response.status {
            ApiVersionErrorStatus::NotFound => GatewayHttpApiVersionErrorStatus::NotFound,
            ApiVersionErrorStatus::Gone => GatewayHttpApiVersionErrorStatus::Gone,
        },
        code: response.code,
        message: response.message,
    }
}

fn requested_api_version_for_path(path: &str, default_version: ApiVersion) -> (ApiVersion, bool) {
    let path = path_without_query_or_fragment(path);
    let first_segment = path
        .strip_prefix('/')
        .unwrap_or(path)
        .split('/')
        .next()
        .unwrap_or_default();
    let Some(version_digits) = first_segment.strip_prefix('v') else {
        return (default_version, false);
    };
    if version_digits.is_empty()
        || !version_digits
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return (default_version, false);
    }
    match version_digits.parse::<u16>() {
        Ok(major) => (ApiVersion::new(major, 0), true),
        Err(_) => (ApiVersion::new(u16::MAX, 0), true),
    }
}
