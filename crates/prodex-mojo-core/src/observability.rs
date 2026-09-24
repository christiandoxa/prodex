use crate::MojoError;

pub const OBSERVABILITY_LABEL_ABI_VERSION: i64 = 1;
const OBSERVABILITY_LABEL_MAX_BYTES: usize = 128;

pub const OPERATIONAL_EVENT_SOURCE_NONE: i64 = 0;
pub const OPERATIONAL_EVENT_SOURCE_REQUEST: i64 = 1;
pub const OPERATIONAL_EVENT_SOURCE_MCP: i64 = 2;
pub const OPERATIONAL_EVENT_SOURCE_AGENT: i64 = 3;
pub const OPERATIONAL_EVENT_SOURCE_ROUTE: i64 = 4;
pub const OPERATIONAL_EVENT_SOURCE_QUOTA: i64 = 5;
pub const OPERATIONAL_EVENT_SOURCE_RETRY: i64 = 6;
pub const OPERATIONAL_EVENT_SOURCE_BACKOFF: i64 = 7;
pub const OPERATIONAL_EVENT_SOURCE_HEALTH: i64 = 8;
pub const OPERATIONAL_EVENT_SOURCE_ERROR: i64 = 9;
pub const OPERATIONAL_EVENT_SOURCE_MODEL: i64 = 10;
pub const OPERATIONAL_EVENT_SOURCE_UPSTREAM: i64 = 11;
pub const OPERATIONAL_EVENT_SOURCE_STREAM: i64 = 12;
pub const OPERATIONAL_EVENT_SOURCE_RESPONSE: i64 = 13;
pub const OPERATIONAL_EVENT_SOURCE_TERMINAL: i64 = 14;
pub const OPERATIONAL_EVENT_SOURCE_TOOL: i64 = 15;
pub const OPERATIONAL_EVENT_SOURCE_LOAD: i64 = 16;
pub const OPERATIONAL_EVENT_SOURCE_SMART: i64 = 17;
pub const OPERATIONAL_EVENT_SOURCE_COMPACT: i64 = 18;
pub const OPERATIONAL_EVENT_SOURCE_EVENT: i64 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationalEventPlan {
    pub source: i64,
    pub interesting: bool,
}

unsafe extern "C" {
    fn prodex_mojo_observability_label_v1(
        abi_version: i64,
        kind: i64,
        value: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_observability_metric_name_v1(
        abi_version: i64,
        plan: i64,
        slot: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_observability_label_key_v1(
        abi_version: i64,
        key: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_operational_event_plan_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        tool_surface_address: u64,
        tool_surface_length: i64,
        tool_surface_present: i64,
        continuation_address: u64,
        continuation_length: i64,
        continuation_present: i64,
        family_address: u64,
        family_length: i64,
        family_present: i64,
        decision_address: u64,
        decision_length: i64,
        decision_present: i64,
        source: u64,
        interesting: u64,
    ) -> i64;
    fn prodex_mojo_operational_event_detail_plan_v1(
        abi_version: i64,
        source_address: u64,
        source_length: i64,
        first_local_chunk: i64,
        output_address: u64,
        output_capacity: i64,
        output_count_address: u64,
    ) -> i64;
}

fn optional_text_parts(value: Option<&str>) -> Result<(u64, i64, i64), MojoError> {
    match value {
        Some(value) => Ok((
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            1,
        )),
        None => Ok((0, 0, 0)),
    }
}

pub fn operational_event_plan(
    event: &str,
    tool_surface: Option<&str>,
    continuation: Option<&str>,
    family: Option<&str>,
    decision: Option<&str>,
) -> Result<OperationalEventPlan, MojoError> {
    if event.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let event_length = i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?;
    let (tool_surface_address, tool_surface_length, tool_surface_present) =
        optional_text_parts(tool_surface)?;
    let (continuation_address, continuation_length, continuation_present) =
        optional_text_parts(continuation)?;
    let (family_address, family_length, family_present) = optional_text_parts(family)?;
    let (decision_address, decision_length, decision_present) = optional_text_parts(decision)?;
    let mut source = -1_i64;
    let mut interesting = -1_i64;
    let status = unsafe {
        prodex_mojo_operational_event_plan_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            event.as_ptr() as usize as u64,
            event_length,
            tool_surface_address,
            tool_surface_length,
            tool_surface_present,
            continuation_address,
            continuation_length,
            continuation_present,
            family_address,
            family_length,
            family_present,
            decision_address,
            decision_length,
            decision_present,
            (&mut source as *mut i64) as usize as u64,
            (&mut interesting as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(OPERATIONAL_EVENT_SOURCE_NONE..=OPERATIONAL_EVENT_SOURCE_EVENT).contains(&source)
        || !matches!(interesting, 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(OperationalEventPlan {
        source,
        interesting: interesting == 1,
    })
}

pub fn operational_event_detail_plan(
    source: &str,
    first_local_chunk: bool,
) -> Result<Vec<i64>, MojoError> {
    const MAX_DETAILS: usize = 64;
    if source.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let source_length = i64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; MAX_DETAILS];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_operational_event_detail_plan_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            source.as_ptr() as usize as u64,
            source_length,
            i64::from(first_local_chunk),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output_count as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > output.len()
        || output[..output_count]
            .iter()
            .any(|value| !(0..=61).contains(value))
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output[..output_count].to_vec())
}

pub fn label(kind: i64, value: i64) -> Result<String, MojoError> {
    if kind < 0 || value < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_label_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            kind,
            value,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn metric_name(plan: i64, slot: i64) -> Result<String, MojoError> {
    if plan < 0 || slot < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_metric_name_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            plan,
            slot,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn label_key(key: i64) -> Result<String, MojoError> {
    if key < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_label_key_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            key,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservabilityPlanLabelSpec {
    pub key: i64,
    pub kind: i64,
}

unsafe extern "C" {
    fn prodex_mojo_observability_plan_label_spec_v1(
        abi_version: i64,
        plan: i64,
        slot: i64,
        key: u64,
        kind: u64,
    ) -> i64;
}

pub fn plan_label_spec(plan: i64, slot: i64) -> Result<ObservabilityPlanLabelSpec, MojoError> {
    if plan < 0 || slot < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut key = -1_i64;
    let mut kind = -1_i64;
    let status = unsafe {
        prodex_mojo_observability_plan_label_spec_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            plan,
            slot,
            (&mut key as *mut i64) as u64,
            (&mut kind as *mut i64) as u64,
        )
    };
    match status {
        0 if key >= 0 && kind >= 0 => Ok(ObservabilityPlanLabelSpec { key, kind }),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_are_bounded_and_reject_unknowns() {
        assert!(label(0, 0).is_ok());
        assert_eq!(label(-1, 0), Err(MojoError::InvalidInput));
        assert_eq!(label(0, -1), Err(MojoError::InvalidInput));
        assert!(label(10_000, 0).is_err());
    }

    #[test]
    fn plan_label_metadata_is_bounded_and_stable() {
        assert_eq!(
            plan_label_spec(68, 0),
            Ok(ObservabilityPlanLabelSpec {
                key: 132,
                kind: 124
            })
        );
        assert_eq!(
            plan_label_spec(63, 4),
            Ok(ObservabilityPlanLabelSpec { key: 82, kind: 118 })
        );
        assert_eq!(
            plan_label_spec(76, 0),
            Ok(ObservabilityPlanLabelSpec { key: 94, kind: 64 })
        );
        assert!(plan_label_spec(68, 9).is_err());
        assert!(plan_label_spec(10_000, 0).is_err());
    }
}
