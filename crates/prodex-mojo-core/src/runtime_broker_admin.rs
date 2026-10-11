use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerAdminOperation {
    Activate,
    ReleaseSessionAffinity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerAdminValueKind {
    ActivationProfile,
    SessionId,
}

unsafe extern "C" {
    fn prodex_runtime_broker_admin_method_v1(
        abi_version: i64,
        operation: i64,
        method_address: u64,
        method_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_broker_admin_value_v1(
        abi_version: i64,
        value_kind: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn operation_code(operation: RuntimeBrokerAdminOperation) -> i64 {
    match operation {
        RuntimeBrokerAdminOperation::Activate => 0,
        RuntimeBrokerAdminOperation::ReleaseSessionAffinity => 1,
    }
}

fn value_kind_code(kind: RuntimeBrokerAdminValueKind) -> i64 {
    match kind {
        RuntimeBrokerAdminValueKind::ActivationProfile => 0,
        RuntimeBrokerAdminValueKind::SessionId => 1,
    }
}

/// Return whether the admin operation received its required POST method.
pub fn method_is_post(
    operation: RuntimeBrokerAdminOperation,
    method: &str,
) -> Result<bool, MojoError> {
    let mut output = [-1_i64];
    let status = unsafe {
        prodex_runtime_broker_admin_method_v1(
            ABI_VERSION,
            operation_code(operation),
            method.as_ptr() as usize as u64,
            signed_len(method)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return the UTF-8 byte span accepted for one admin string value.
pub fn value_span(
    kind: RuntimeBrokerAdminValueKind,
    value: &str,
) -> Result<Option<(usize, usize)>, MojoError> {
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_broker_admin_value_v1(
            ABI_VERSION,
            value_kind_code(kind),
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    let valid = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if !valid {
        return Ok(None);
    }
    let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end
        || end > value.len()
        || !value.is_char_boundary(start)
        || !value.is_char_boundary(end)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_policy_checks_each_admin_operation() {
        for operation in [
            RuntimeBrokerAdminOperation::Activate,
            RuntimeBrokerAdminOperation::ReleaseSessionAffinity,
        ] {
            assert!(method_is_post(operation, "POST").unwrap());
            assert!(!method_is_post(operation, "GET").unwrap());
        }
    }

    #[test]
    fn value_policy_trims_unicode_whitespace_and_enforces_byte_limit() {
        assert_eq!(
            value_span(
                RuntimeBrokerAdminValueKind::ActivationProfile,
                "\u{2003}work\u{2003}"
            )
            .unwrap()
            .map(|(start, end)| &"\u{2003}work\u{2003}"[start..end]),
            Some("work")
        );
        assert_eq!(
            value_span(RuntimeBrokerAdminValueKind::SessionId, " \t").unwrap(),
            None
        );
        assert_eq!(
            value_span(RuntimeBrokerAdminValueKind::SessionId, &"a".repeat(257)).unwrap(),
            None
        );
        assert_eq!(
            value_span(RuntimeBrokerAdminValueKind::SessionId, &"a".repeat(256)).unwrap(),
            Some((0, 256))
        );
    }
}
