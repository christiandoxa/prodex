use crate::MojoError;

const ABI_VERSION: i64 = 1;

/// Parsed admin pagination or the original first query error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerSnapshotQuery {
    Query { after: u64, limit: usize },
    Malformed,
    InvalidCursor,
    InvalidLimit,
    LimitOutOfBounds,
}

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
    fn prodex_runtime_broker_snapshot_query_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        max_entries: u64,
        output_address: u64,
    ) -> i64;
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

/// Parse pagination in the existing compiled broker admin owner. Duplicate
/// keys retain last-value precedence, without URL decoding or whitespace trim.
pub fn log_snapshot_query(
    url: &str,
    max_entries: usize,
) -> Result<RuntimeBrokerSnapshotQuery, MojoError> {
    let mut output = [u64::MAX; 3];
    match unsafe {
        prodex_runtime_broker_snapshot_query_v1(
            ABI_VERSION,
            url.as_ptr() as usize as u64,
            signed_len(url)?,
            max_entries as u64,
            output.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output[0] {
        0 => {
            let limit = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
            if limit == 0 || limit > max_entries {
                return Err(MojoError::InvalidOutput);
            }
            Ok(RuntimeBrokerSnapshotQuery::Query {
                after: output[1],
                limit,
            })
        }
        1 => Ok(RuntimeBrokerSnapshotQuery::Malformed),
        2 => Ok(RuntimeBrokerSnapshotQuery::InvalidCursor),
        3 => Ok(RuntimeBrokerSnapshotQuery::InvalidLimit),
        4 => Ok(RuntimeBrokerSnapshotQuery::LimitOutOfBounds),
        _ => Err(MojoError::InvalidOutput),
    }
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
#[path = "runtime_broker_admin/snapshot_query_tests.rs"]
mod snapshot_query_tests;

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
