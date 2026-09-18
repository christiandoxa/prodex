unsafe extern "C" {
    fn prodex_mojo_governance_finding_classification_v1(
        abi_version: i64,
        mode: i64,
        values: u64,
        value_count: i64,
        classification: i64,
        output: u64,
    ) -> i64;
}

fn finding_classification(
    mode: i64,
    values: &[i64],
    value_count: usize,
    classification: u8,
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_finding_classification_v1(
            6,
            mode,
            values.as_ptr() as u64,
            i64::try_from(value_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(classification),
            (&mut output as *mut i64) as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_finding_minimum_classification(kind: u8) -> Result<u8, crate::MojoError> {
    let value = u8::try_from(finding_classification(0, &[i64::from(kind)], 1, 0)?)
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    (value <= 3)
        .then_some(value)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn governance_findings_exceed_classification(
    kinds: &[u8],
    classification: u8,
) -> Result<bool, crate::MojoError> {
    let values = kinds
        .iter()
        .map(|kind| i64::from(*kind))
        .collect::<Vec<_>>();
    match finding_classification(1, &values, values.len(), classification)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    governance_finding_minimum_classification(0).is_ok()
        && governance_findings_exceed_classification(&[0], 3).is_ok()
}
