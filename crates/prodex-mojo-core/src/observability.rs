use crate::MojoError;

pub const OBSERVABILITY_LABEL_ABI_VERSION: i64 = 1;
const OBSERVABILITY_LABEL_MAX_BYTES: usize = 128;

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
