use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct ArgumentView {
    address: u64,
    length: u64,
    valid_utf8: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayConfigAssignmentViolation {
    MissingValue,
    ValueNotUtf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayTransportFlags {
    pub remote: bool,
    pub no_daemon: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshProjectionAction {
    Keep(usize),
    ConfigArg(usize),
    Feature { index: usize, enabled: bool },
}

unsafe extern "C" {
    fn prodex_runtime_overlay_config_assignments_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        records_address: u64,
        record_capacity: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_overlay_workspace_trust_indices_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        indices_address: u64,
        index_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_overlay_transport_flags_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_overlay_fresh_projection_v1(
        abi_version: i64,
        arguments_address: u64,
        count: i64,
        records_address: u64,
        record_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn views(arguments: &[Option<&str>]) -> Result<Vec<ArgumentView>, MojoError> {
    arguments
        .iter()
        .map(|argument| match argument {
            Some(value) => Ok(ArgumentView {
                address: value.as_ptr() as usize as u64,
                length: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
                valid_utf8: 1,
            }),
            None => Ok(ArgumentView {
                address: 0,
                length: 0,
                valid_utf8: 0,
            }),
        })
        .collect()
}

fn validate_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn argument_slice(
    arguments: &[Option<&str>],
    index: i64,
    offset: i64,
    length: i64,
) -> Result<String, MojoError> {
    let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
    let value = arguments
        .get(index)
        .and_then(|value| *value)
        .ok_or(MojoError::InvalidOutput)?;
    let offset = usize::try_from(offset).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = offset.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    value
        .get(offset..end)
        .map(str::to_string)
        .ok_or(MojoError::InvalidOutput)
}

pub fn config_assignments(
    arguments: &[Option<&str>],
) -> Result<Result<Vec<String>, OverlayConfigAssignmentViolation>, MojoError> {
    let views = views(arguments)?;
    let capacity = arguments.len();
    let mut records = vec![-1_i64; capacity.saturating_mul(3)];
    let mut result = [0_i64, -1_i64];
    let status = unsafe {
        prodex_runtime_overlay_config_assignments_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    match result[0] {
        1 => return Ok(Err(OverlayConfigAssignmentViolation::MissingValue)),
        2 => return Ok(Err(OverlayConfigAssignmentViolation::ValueNotUtf8)),
        0 => {}
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
    if written > capacity {
        return Err(MojoError::InvalidOutput);
    }
    (0..written)
        .map(|index| {
            let base = index * 3;
            argument_slice(
                arguments,
                records[base],
                records[base + 1],
                records[base + 2],
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Ok)
}

pub fn workspace_trust_indices(arguments: &[Option<&str>]) -> Result<Vec<usize>, MojoError> {
    let views = views(arguments)?;
    let mut indices = vec![-1_i64; arguments.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_overlay_workspace_trust_indices_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    validate_status(status)?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > indices.len() {
        return Err(MojoError::InvalidOutput);
    }
    indices.truncate(written);
    indices
        .into_iter()
        .map(|index| usize::try_from(index).map_err(|_| MojoError::InvalidOutput))
        .collect()
}

pub fn transport_flags(arguments: &[Option<&str>]) -> Result<OverlayTransportFlags, MojoError> {
    let views = views(arguments)?;
    let mut result = [0_i64; 2];
    let status = unsafe {
        prodex_runtime_overlay_transport_flags_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    validate_status(status)?;
    let remote = match result[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let no_daemon = match result[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(OverlayTransportFlags { remote, no_daemon })
}

pub fn fresh_projection(
    arguments: &[Option<&str>],
) -> Result<Vec<FreshProjectionAction>, MojoError> {
    if arguments.iter().any(Option::is_none) {
        return Err(MojoError::InvalidInput);
    }
    let views = views(arguments)?;
    let capacity = arguments.len();
    let mut records = vec![-1_i64; capacity.saturating_mul(3)];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_overlay_fresh_projection_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    validate_status(status)?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > capacity {
        return Err(MojoError::InvalidOutput);
    }
    (0..written)
        .map(|record| {
            let base = record * 3;
            let index = usize::try_from(records[base + 1]).map_err(|_| MojoError::InvalidOutput)?;
            if index >= arguments.len() {
                return Err(MojoError::InvalidOutput);
            }
            match records[base] {
                0 => Ok(FreshProjectionAction::Keep(index)),
                1 => Ok(FreshProjectionAction::ConfigArg(index)),
                2 => {
                    let enabled = match records[base + 2] {
                        0 => false,
                        1 => true,
                        _ => return Err(MojoError::InvalidOutput),
                    };
                    Ok(FreshProjectionAction::Feature { index, enabled })
                }
                _ => Err(MojoError::InvalidOutput),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_overlay_policy_preserves_cli_scanning_contracts() {
        let args = [
            Some("-c"),
            Some("model='first'"),
            Some("--config=projects={a=1}"),
            Some("-cdisable_paste_burst=true"),
        ];
        assert_eq!(
            config_assignments(&args).unwrap().unwrap(),
            vec![
                "model='first'".to_string(),
                "projects={a=1}".to_string(),
                "disable_paste_burst=true".to_string(),
            ]
        );
        assert_eq!(
            config_assignments(&[Some("-c")]).unwrap(),
            Err(OverlayConfigAssignmentViolation::MissingValue)
        );
        assert_eq!(
            config_assignments(&[Some("-c"), None]).unwrap(),
            Err(OverlayConfigAssignmentViolation::ValueNotUtf8)
        );

        let trust = [
            Some("-c"),
            Some(" projects={one=1}"),
            Some("--config= projects={two=2}"),
            Some("-c==  projects={three=3}"),
            Some("-c"),
            Some("model='ignored'"),
        ];
        assert_eq!(workspace_trust_indices(&trust).unwrap(), vec![0, 1, 2, 3]);

        let flags = [
            Some("--remote=unix:///tmp/x.sock"),
            Some("resume"),
            Some("--no-daemon"),
        ];
        assert_eq!(
            transport_flags(&flags).unwrap(),
            OverlayTransportFlags {
                remote: true,
                no_daemon: true,
            }
        );

        let fresh = [
            Some("-c"),
            Some("model='x'"),
            Some("--enable"),
            Some("foo"),
            Some("--dangerously-bypass-approvals-and-sandbox"),
            Some("--dangerously-bypass-hook-trust"),
            Some("--config=bar=true"),
            Some("resume"),
        ];
        assert_eq!(
            fresh_projection(&fresh).unwrap(),
            vec![
                FreshProjectionAction::ConfigArg(0),
                FreshProjectionAction::ConfigArg(1),
                FreshProjectionAction::Feature {
                    index: 3,
                    enabled: true,
                },
                FreshProjectionAction::Keep(5),
                FreshProjectionAction::ConfigArg(6),
                FreshProjectionAction::Keep(7),
            ]
        );
    }
}
