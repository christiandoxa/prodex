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
    fn prodex_mojo_governance_inspection_order_v1(
        abi_version: i64,
        findings: u64,
        findings_count: i64,
        tags: u64,
        tags_count: i64,
        reason_codes: u64,
        reason_codes_count: i64,
        finding_order: u64,
        tag_order: u64,
        tag_count: u64,
        reason_code_order: u64,
        reason_code_count: u64,
    ) -> i64;
}

const GOVERNANCE_INSPECTION_ORDER_ABI_VERSION: i64 = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GovernanceTextView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GovernanceFindingOrderKey {
    field_path: GovernanceTextView,
    start_byte: i64,
    end_byte: i64,
    kind: i64,
    detector_id: GovernanceTextView,
    confidence_basis_points: i64,
}

/// Finding fields used to order an inspection result without moving domain types into this crate.
#[derive(Debug, Clone, Copy)]
pub struct GovernanceInspectionFindingOrderKey<'a> {
    /// Validated content path used as the first sort key.
    pub field_path: &'a str,
    /// Inclusive start byte offset within the source content.
    pub start_byte: u32,
    /// Exclusive end byte offset within the source content.
    pub end_byte: u32,
    /// Stable finding-kind tag.
    pub kind: u8,
    /// Validated detector identifier used as a tie-breaker.
    pub detector_id: &'a str,
    /// Confidence value used as the final sort key.
    pub confidence_basis_points: u16,
}

/// Validated index order for findings, tags, and reason codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceInspectionOrder {
    /// Every finding input index, in governance sort order.
    pub finding_indices: Vec<usize>,
    /// Unique tag input indices, in bytewise lexical order.
    pub tag_indices: Vec<usize>,
    /// Unique reason-code input indices, in bytewise lexical order.
    pub reason_code_indices: Vec<usize>,
}

fn governance_text_view(value: &str) -> GovernanceTextView {
    GovernanceTextView {
        ptr: value.as_ptr() as usize as u64,
        len: value.len() as u64,
    }
}

fn validate_governance_order(
    values: &[i64],
    input_count: usize,
    require_full_order: bool,
) -> Result<Vec<usize>, crate::MojoError> {
    if values.len() > input_count || (require_full_order && values.len() != input_count) {
        return Err(crate::MojoError::InvalidOutput);
    }
    let mut seen = vec![false; input_count];
    let mut order = Vec::with_capacity(values.len());
    for value in values {
        let index = usize::try_from(*value).map_err(|_| crate::MojoError::InvalidOutput)?;
        let Some(has_seen) = seen.get_mut(index) else {
            return Err(crate::MojoError::InvalidOutput);
        };
        if *has_seen {
            return Err(crate::MojoError::InvalidOutput);
        }
        *has_seen = true;
        order.push(index);
    }
    Ok(order)
}

/// Ask the governance kernel for finding order and sorted unique metadata indices.
///
/// String ordering matches Rust UTF-8 bytewise lexical ordering. Duplicate tag
/// and reason-code strings retain one original index.
pub fn governance_inspection_order(
    findings: &[GovernanceInspectionFindingOrderKey<'_>],
    tags: &[&str],
    reason_codes: &[&str],
) -> Result<GovernanceInspectionOrder, crate::MojoError> {
    let findings_input = findings
        .iter()
        .map(|finding| GovernanceFindingOrderKey {
            field_path: governance_text_view(finding.field_path),
            start_byte: i64::from(finding.start_byte),
            end_byte: i64::from(finding.end_byte),
            kind: i64::from(finding.kind),
            detector_id: governance_text_view(finding.detector_id),
            confidence_basis_points: i64::from(finding.confidence_basis_points),
        })
        .collect::<Vec<_>>();
    let tag_views = tags
        .iter()
        .map(|value| governance_text_view(value))
        .collect::<Vec<_>>();
    let reason_code_views = reason_codes
        .iter()
        .map(|value| governance_text_view(value))
        .collect::<Vec<_>>();
    let findings_count =
        i64::try_from(findings_input.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let tags_count = i64::try_from(tag_views.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let reason_codes_count =
        i64::try_from(reason_code_views.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let mut finding_order = vec![0_i64; findings.len()];
    let mut tag_order = vec![0_i64; tags.len()];
    let mut reason_code_order = vec![0_i64; reason_codes.len()];
    let mut tag_count = -1_i64;
    let mut reason_code_count = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_inspection_order_v1(
            GOVERNANCE_INSPECTION_ORDER_ABI_VERSION,
            findings_input.as_ptr() as usize as u64,
            findings_count,
            tag_views.as_ptr() as usize as u64,
            tags_count,
            reason_code_views.as_ptr() as usize as u64,
            reason_codes_count,
            finding_order.as_mut_ptr() as usize as u64,
            tag_order.as_mut_ptr() as usize as u64,
            (&mut tag_count as *mut i64) as usize as u64,
            reason_code_order.as_mut_ptr() as usize as u64,
            (&mut reason_code_count as *mut i64) as usize as u64,
        )
    };
    governance_status(status)?;
    let tag_count = usize::try_from(tag_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    let reason_code_count =
        usize::try_from(reason_code_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    if tag_count > tag_order.len() || reason_code_count > reason_code_order.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(GovernanceInspectionOrder {
        finding_indices: validate_governance_order(&finding_order, findings.len(), true)?,
        tag_indices: validate_governance_order(&tag_order[..tag_count], tags.len(), false)?,
        reason_code_indices: validate_governance_order(
            &reason_code_order[..reason_code_count],
            reason_codes.len(),
            false,
        )?,
    })
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
        && governance_inspection_order(&[], &[], &[]).is_ok()
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

    #[test]
    fn governance_inspection_order_enforces_mojo_collection_caps() {
        let finding = GovernanceInspectionFindingOrderKey {
            field_path: "$.input",
            start_byte: 0,
            end_byte: 0,
            kind: 0,
            detector_id: "detector.a",
            confidence_basis_points: 0,
        };
        let findings = vec![finding; 256];
        let tags = ["alpha"; 32];
        let reason_codes = ["reason.a"; 32];
        let order = governance_inspection_order(&findings, &tags, &reason_codes).unwrap();
        assert_eq!(order.finding_indices.len(), findings.len());
        assert_eq!(order.tag_indices.len(), 1);
        assert_eq!(order.reason_code_indices.len(), 1);

        assert!(matches!(
            governance_inspection_order(&vec![finding; 257], &[], &[]),
            Err(crate::MojoError::Capacity)
        ));
        assert!(matches!(
            governance_inspection_order(&[], &["alpha"; 33], &[]),
            Err(crate::MojoError::Capacity)
        ));
        assert!(matches!(
            governance_inspection_order(&[], &[], &["reason.a"; 33]),
            Err(crate::MojoError::Capacity)
        ));
    }
}
