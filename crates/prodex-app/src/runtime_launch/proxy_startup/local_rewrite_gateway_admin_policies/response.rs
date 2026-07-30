use super::{
    ApplicationGovernanceLifecycleError, GovernanceRepositoryError,
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_json_error_response,
    build_runtime_proxy_response_from_parts,
};

pub(super) fn lifecycle_repository_error(
    error: ApplicationGovernanceLifecycleError,
) -> GovernanceRepositoryError {
    match error {
        ApplicationGovernanceLifecycleError::InvalidAction => {
            GovernanceRepositoryError::InvalidInput
        }
        ApplicationGovernanceLifecycleError::Repository(error) => error,
    }
}

pub(super) fn lifecycle_error(
    error: ApplicationGovernanceLifecycleError,
) -> tiny_http::ResponseBox {
    repository_error(lifecycle_repository_error(error))
}

pub(super) fn repository_error(error: GovernanceRepositoryError) -> tiny_http::ResponseBox {
    let (status, code, message) = match error {
        GovernanceRepositoryError::InvalidInput => (
            400,
            "governance_policy_invalid",
            "policy governance request is invalid",
        ),
        GovernanceRepositoryError::TenantMismatch => (
            403,
            "governance_policy_forbidden",
            "policy governance request is forbidden",
        ),
        GovernanceRepositoryError::NotFound => (
            404,
            "governance_policy_not_found",
            "policy governance resource was not found",
        ),
        GovernanceRepositoryError::EtagMismatch => (
            412,
            "governance_policy_precondition_failed",
            "policy governance precondition failed",
        ),
        GovernanceRepositoryError::ApprovalSelfAction => (
            403,
            "governance_policy_self_approval_forbidden",
            "policy maker cannot approve this revision",
        ),
        GovernanceRepositoryError::StaleVersion => (
            409,
            "governance_policy_version_stale",
            "policy approval version is stale",
        ),
        GovernanceRepositoryError::Conflict
        | GovernanceRepositoryError::InvalidTransition
        | GovernanceRepositoryError::ApprovalRequired => (
            409,
            "governance_policy_conflict",
            "policy governance state conflicts with this request",
        ),
        GovernanceRepositoryError::SnapshotUnavailable => (
            422,
            "governance_policy_snapshot_invalid",
            "policy revision could not be verified",
        ),
        GovernanceRepositoryError::Unsupported => (
            501,
            "governance_policy_operation_unsupported",
            "policy governance operation is not supported by this backend",
        ),
        GovernanceRepositoryError::Database | GovernanceRepositoryError::AuditChainConflict => (
            503,
            "governance_policy_storage_unavailable",
            "policy governance storage is temporarily unavailable",
        ),
    };
    build_runtime_proxy_json_error_response(status, code, message)
}

pub(super) fn invalid_request() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        400,
        "governance_policy_invalid",
        "policy governance request is invalid",
    )
}

pub(super) fn json_response_with_etag(
    status: u16,
    value: serde_json::Value,
    etag: &str,
) -> tiny_http::ResponseBox {
    let body = serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec());
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![
            (
                "content-type".to_string(),
                b"application/json; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
            ("etag".to_string(), etag.as_bytes().to_vec()),
        ],
        body: body.into(),
    })
}
