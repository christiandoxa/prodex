//! Typed, fail-closed response planning for domain identifier and secret errors.
//! Only trusted error tags enter the compiled ABI; user identifiers and
//! secret material are never marshalled into the policy kernel.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const CODE_LABELS: [&str; 17] = [
    "identifier_invalid",
    "tenant_id_invalid",
    "principal_id_invalid",
    "request_id_invalid",
    "call_id_invalid",
    "reservation_id_invalid",
    "virtual_key_id_invalid",
    "role_binding_id_invalid",
    "provider_credential_id_invalid",
    "policy_revision_id_invalid",
    "audit_event_id_invalid",
    "secret_rotation_max_age_invalid",
    "secret_rotation_overlap_invalid",
    "secret_not_found",
    "secret_permission_denied",
    "secret_provider_unavailable",
    "secret_version_stale",
];
const MESSAGE_LABELS: [&str; 5] = [
    "identifier is invalid",
    "secret rotation policy is invalid",
    "secret could not be resolved",
    "secret provider is unavailable",
    "secret version is stale",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainErrorStatus {
    InvalidRequest,
    Forbidden,
    NotFound,
    ServiceUnavailable,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainErrorPlan {
    pub status: DomainErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

unsafe extern "C" {
    fn prodex_mojo_domain_error_plan_v1(
        abi_version: i64,
        operation: i64,
        kind_address: u64,
        kind_length: i64,
        error_tag: i64,
        output_address: u64,
    ) -> i64;
}

fn plan(operation: i64, kind: Option<&str>, error_tag: i64) -> Result<DomainErrorPlan, MojoError> {
    let mut result = [-1_i64; 3];
    let (address, length) = if let Some(kind) = kind {
        (
            kind.as_ptr() as usize as u64,
            i64::try_from(kind.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } else {
        (0, 0)
    };
    match unsafe {
        prodex_mojo_domain_error_plan_v1(
            ABI_VERSION,
            operation,
            address,
            length,
            error_tag,
            result.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let status = match result[0] {
        0 => DomainErrorStatus::InvalidRequest,
        1 => DomainErrorStatus::Forbidden,
        2 => DomainErrorStatus::NotFound,
        3 => DomainErrorStatus::ServiceUnavailable,
        4 => DomainErrorStatus::Conflict,
        _ => return Err(MojoError::InvalidOutput),
    };
    let code = CODE_LABELS
        .get(usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?)
        .copied()
        .ok_or(MojoError::InvalidOutput)?;
    let message = MESSAGE_LABELS
        .get(usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?)
        .copied()
        .ok_or(MojoError::InvalidOutput)?;
    Ok(DomainErrorPlan {
        status,
        code,
        message,
    })
}

pub fn identifier_error_plan(kind: &str) -> Result<DomainErrorPlan, MojoError> {
    plan(0, Some(kind), 0)
}

pub fn secret_rotation_error_plan(tag: i64) -> Result<DomainErrorPlan, MojoError> {
    plan(1, None, tag)
}

pub fn secret_resolution_error_plan(tag: i64) -> Result<DomainErrorPlan, MojoError> {
    plan(2, None, tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_domain_error_identifier_kind_classification_is_exact() {
        for (kind, expected) in [
            ("tenant_id", "tenant_id_invalid"),
            ("principal_id", "principal_id_invalid"),
            ("request_id", "request_id_invalid"),
            ("call_id", "call_id_invalid"),
            ("reservation_id", "reservation_id_invalid"),
            ("virtual_key_id", "virtual_key_id_invalid"),
            ("role_binding_id", "role_binding_id_invalid"),
            ("provider_credential_id", "provider_credential_id_invalid"),
            ("policy_revision_id", "policy_revision_id_invalid"),
            ("audit_event_id", "audit_event_id_invalid"),
            ("", "identifier_invalid"),
            ("Tenant_id", "identifier_invalid"),
            ("tenant_id_extra", "identifier_invalid"),
            ("東京", "identifier_invalid"),
        ] {
            assert_eq!(
                identifier_error_plan(kind).unwrap(),
                DomainErrorPlan {
                    status: DomainErrorStatus::InvalidRequest,
                    code: expected,
                    message: "identifier is invalid",
                },
                "{kind}"
            );
        }
    }

    #[test]
    fn compiled_domain_error_secret_variants_keep_status_and_redacted_messages() {
        for (tag, code) in [
            (0, "secret_rotation_max_age_invalid"),
            (1, "secret_rotation_overlap_invalid"),
        ] {
            assert_eq!(
                secret_rotation_error_plan(tag).unwrap(),
                DomainErrorPlan {
                    status: DomainErrorStatus::InvalidRequest,
                    code,
                    message: "secret rotation policy is invalid",
                }
            );
        }
        for (tag, status, code, message) in [
            (
                0,
                DomainErrorStatus::NotFound,
                "secret_not_found",
                "secret could not be resolved",
            ),
            (
                1,
                DomainErrorStatus::Forbidden,
                "secret_permission_denied",
                "secret could not be resolved",
            ),
            (
                2,
                DomainErrorStatus::ServiceUnavailable,
                "secret_provider_unavailable",
                "secret provider is unavailable",
            ),
            (
                3,
                DomainErrorStatus::Conflict,
                "secret_version_stale",
                "secret version is stale",
            ),
        ] {
            assert_eq!(
                secret_resolution_error_plan(tag).unwrap(),
                DomainErrorPlan {
                    status,
                    code,
                    message
                },
            );
        }
        assert_eq!(secret_rotation_error_plan(-1), Err(MojoError::InvalidInput));
        assert_eq!(secret_rotation_error_plan(2), Err(MojoError::InvalidInput));
        assert_eq!(
            secret_resolution_error_plan(4),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn compiled_domain_error_abi_rejects_invalid_input_without_writing() {
        let mut output = [57_i64; 3];
        let address = output.as_mut_ptr() as usize as u64;
        let fixture = "tenant_id";
        for (abi, operation, pointer, length, tag, expected) in [
            (2, 0, fixture.as_ptr() as usize as u64, 9, 0, 4),
            (1, 3, fixture.as_ptr() as usize as u64, 9, 0, 1),
            (1, 0, fixture.as_ptr() as usize as u64, -1, 0, 1),
            (1, 0, 0, 9, 0, 1),
            (1, 0, fixture.as_ptr() as usize as u64, 9, 1, 1),
            (1, 1, fixture.as_ptr() as usize as u64, 9, 0, 1),
            (1, 2, 0, 0, 10, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_domain_error_plan_v1(abi, operation, pointer, length, tag, address)
                },
                expected,
            );
            assert_eq!(output, [57; 3]);
        }
        assert_eq!(
            unsafe { prodex_mojo_domain_error_plan_v1(1, 0, 0, 0, 0, 0) },
            1,
        );
        assert_eq!(output, [57; 3]);
    }
}
