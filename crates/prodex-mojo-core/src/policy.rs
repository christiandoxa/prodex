unsafe extern "C" {
    fn prodex_mojo_governance_finding_classification_v1(
        abi_version: i64,
        mode: i64,
        values: u64,
        value_count: i64,
        classification: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_label_v1(
        abi_version: i64,
        kind: i64,
        value: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_governance_coverage_combine_v1(
        abi_version: i64,
        left: i64,
        right: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_text_valid_v1(
        abi_version: i64,
        kind: i64,
        address: u64,
        length: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_limits_valid_v1(
        abi_version: i64,
        max_detectors: u64,
        max_findings: u64,
        max_tags: u64,
        max_reason_codes: u64,
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

fn governance_status(status: i64) -> Result<(), crate::MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(crate::MojoError::InvalidInput),
        2 => Err(crate::MojoError::Capacity),
        4 => Err(crate::MojoError::AbiMismatch),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

fn load_governance_label(kind: i64, value: i64) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; 32];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_label_v1(
            6,
            kind,
            value,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    governance_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

fn cached_governance_label(
    kind: i64,
    value: u8,
    count: usize,
    cache: &'static std::sync::OnceLock<Result<Vec<String>, crate::MojoError>>,
) -> Result<&'static str, crate::MojoError> {
    let value = usize::from(value);
    if value >= count {
        return Err(crate::MojoError::InvalidInput);
    }
    match cache.get_or_init(|| {
        (0..count)
            .map(|value| {
                let value = i64::try_from(value).map_err(|_| crate::MojoError::InvalidInput)?;
                load_governance_label(kind, value)
            })
            .collect()
    }) {
        Ok(labels) => labels
            .get(value)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn governance_classification_label(value: u8) -> Result<&'static str, crate::MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, crate::MojoError>> =
        std::sync::OnceLock::new();
    cached_governance_label(0, value, 4, &LABELS)
}

pub fn governance_coverage_label(value: u8) -> Result<&'static str, crate::MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, crate::MojoError>> =
        std::sync::OnceLock::new();
    cached_governance_label(1, value, 3, &LABELS)
}

pub fn governance_coverage_combine(left: u8, right: u8) -> Result<u8, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_coverage_combine_v1(
            6,
            i64::from(left),
            i64::from(right),
            (&mut output as *mut i64) as usize as u64,
        )
    };
    governance_status(status)?;
    let output = u8::try_from(output).map_err(|_| crate::MojoError::InvalidOutput)?;
    (output <= 2)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn governance_text_valid(kind: i64, value: &str) -> Result<bool, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_text_valid_v1(
            6,
            kind,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    governance_status(status)?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_content_location_path_valid(value: &str) -> Result<bool, crate::MojoError> {
    governance_text_valid(0, value)
}

pub fn governance_inspection_token_valid(value: &str) -> Result<bool, crate::MojoError> {
    governance_text_valid(1, value)
}

pub fn governance_inspection_limits_valid(
    max_detectors: usize,
    max_findings: usize,
    max_tags: usize,
    max_reason_codes: usize,
) -> Result<bool, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_limits_valid_v1(
            6,
            u64::try_from(max_detectors).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(max_findings).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(max_tags).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(max_reason_codes).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    governance_status(status)?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
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
        && governance_classification_label(0).is_ok()
        && governance_coverage_label(0).is_ok()
        && governance_coverage_combine(0, 2).is_ok()
        && governance_content_location_path_valid("$.input[0]").is_ok()
        && governance_inspection_token_valid("detector.v1").is_ok()
        && governance_inspection_limits_valid(8, 256, 32, 32).is_ok()
}

#[cfg(test)]
mod governance_policy_tests {
    use super::*;

    #[test]
    fn governance_inspection_policy_contracts_are_mojo_owned() {
        assert_eq!(governance_classification_label(0).unwrap(), "public");
        assert_eq!(governance_classification_label(3).unwrap(), "restricted");
        assert_eq!(governance_coverage_label(0).unwrap(), "full");
        assert_eq!(governance_coverage_label(2).unwrap(), "unsupported");
        assert_eq!(governance_coverage_combine(0, 0).unwrap(), 0);
        assert_eq!(governance_coverage_combine(0, 2).unwrap(), 1);
        assert_eq!(governance_coverage_combine(2, 2).unwrap(), 2);
        assert!(governance_content_location_path_valid("$.input[0].content").unwrap());
        assert!(!governance_content_location_path_valid("$.bad value").unwrap());
        assert!(governance_inspection_token_valid("detector.v1:local/foo").unwrap());
        assert!(!governance_inspection_token_valid("detector secret").unwrap());
        assert!(governance_inspection_limits_valid(8, 256, 32, 32).unwrap());
        assert!(!governance_inspection_limits_valid(0, 256, 32, 32).unwrap());
        assert!(!governance_inspection_limits_valid(9, 256, 32, 32).unwrap());
    }
}
