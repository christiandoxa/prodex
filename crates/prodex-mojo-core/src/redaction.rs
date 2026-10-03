use crate::MojoError;

const REDACTION_ABI_VERSION: i64 = 1;
const REDACTION_LOCAL_PLACEHOLDER_EXPANSION: usize = 9;

/// Finding-kind tag used by the local-redaction Mojo ABI.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalInspectionFindingKind {
    EmailAddress = 0,
    PhoneNumber = 1,
    PersonName = 2,
    PhysicalAddress = 3,
    GovernmentId = 4,
    FinancialAccount = 5,
    PaymentCard = 6,
    AccessToken = 7,
    ApiKey = 8,
    PrivateKey = 9,
    Password = 10,
    TenantSensitive = 11,
}

impl LocalInspectionFindingKind {
    fn from_abi_tag(value: i64) -> Result<Self, MojoError> {
        match value {
            0 => Ok(Self::EmailAddress),
            1 => Ok(Self::PhoneNumber),
            2 => Ok(Self::PersonName),
            3 => Ok(Self::PhysicalAddress),
            4 => Ok(Self::GovernmentId),
            5 => Ok(Self::FinancialAccount),
            6 => Ok(Self::PaymentCard),
            7 => Ok(Self::AccessToken),
            8 => Ok(Self::ApiKey),
            9 => Ok(Self::PrivateKey),
            10 => Ok(Self::Password),
            11 => Ok(Self::TenantSensitive),
            _ => Err(MojoError::InvalidOutput),
        }
    }
}

unsafe extern "C" {
    fn prodex_redaction_json_field_plan_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_redaction_key_sensitive_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
    fn prodex_redaction_secret_like_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        scratch_one_address: u64,
        scratch_two_address: u64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_redaction_presidio_transport_policy_v1(
        abi_version: i64,
        operation: i64,
        input0: i64,
        input1: i64,
        input2: i64,
        input3: i64,
        input4: i64,
    ) -> i64;
    fn prodex_redaction_gateway_extra_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        scratch_address: u64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_redaction_local_inspection_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        sensitive_kind: i64,
        output_address: u64,
        output_capacity: i64,
        match_output_address: u64,
        match_capacity: i64,
        match_count_address: u64,
        written_address: u64,
    ) -> i64;
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn buffer_capacity(input_len: usize) -> Result<usize, MojoError> {
    input_len
        .checked_mul(3)
        .and_then(|value| value.checked_add(32))
        .map(|value| value.max(32))
        .ok_or(MojoError::Capacity)
}

fn output_string(output: Vec<u8>, written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonInspectionMode {
    SchemaOnly,
    DirectStrings,
    AllStrings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonSensitiveKind {
    None,
    PrivateKey,
    ApiKey,
    AccessToken,
    Password,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonFieldPlan {
    pub inspection_mode: JsonInspectionMode,
    pub sensitive_kind: JsonSensitiveKind,
    pub known_protocol_metadata: bool,
    pub unsupported_modality: bool,
    pub tools_field: bool,
}

pub fn json_field_plan(value: &str) -> Result<JsonFieldPlan, MojoError> {
    let mut output = [-1_i64; 5];
    status(unsafe {
        prodex_redaction_json_field_plan_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let inspection_mode = match output[0] {
        0 => JsonInspectionMode::SchemaOnly,
        1 => JsonInspectionMode::DirectStrings,
        2 => JsonInspectionMode::AllStrings,
        _ => return Err(MojoError::InvalidOutput),
    };
    let sensitive_kind = match output[1] {
        0 => JsonSensitiveKind::None,
        1 => JsonSensitiveKind::PrivateKey,
        2 => JsonSensitiveKind::ApiKey,
        3 => JsonSensitiveKind::AccessToken,
        4 => JsonSensitiveKind::Password,
        _ => return Err(MojoError::InvalidOutput),
    };
    let bool_output = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    Ok(JsonFieldPlan {
        inspection_mode,
        sensitive_kind,
        known_protocol_metadata: bool_output(output[2])?,
        unsupported_modality: bool_output(output[3])?,
        tools_field: bool_output(output[4])?,
    })
}

fn presidio_transport_policy(operation: i64, inputs: [bool; 5]) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_redaction_presidio_transport_policy_v1(
            REDACTION_ABI_VERSION,
            operation,
            i64::from(inputs[0]),
            i64::from(inputs[1]),
            i64::from(inputs[2]),
            i64::from(inputs[3]),
            i64::from(inputs[4]),
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn presidio_fail_closed(
    rollout_enforce: bool,
    governance_enforcing: bool,
    legacy_local_enabled: bool,
    tenant_detector_enabled: bool,
    explicit_fail_closed: bool,
) -> Result<bool, MojoError> {
    presidio_transport_policy(
        0,
        [
            rollout_enforce,
            governance_enforcing,
            legacy_local_enabled,
            tenant_detector_enabled,
            explicit_fail_closed,
        ],
    )
}

pub fn presidio_local_inspection_required(
    rollout_off: bool,
    governance_enforcing: bool,
    legacy_local_enabled: bool,
    configured_detector_enabled: bool,
) -> Result<bool, MojoError> {
    presidio_transport_policy(
        1,
        [
            rollout_off,
            governance_enforcing,
            legacy_local_enabled,
            configured_detector_enabled,
            false,
        ],
    )
}

pub fn presidio_external_coverage_denied(
    fail_closed: bool,
    coverage_full: bool,
) -> Result<bool, MojoError> {
    presidio_transport_policy(2, [fail_closed, coverage_full, false, false, false])
}

pub fn key_looks_sensitive(value: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_redaction_key_sensitive_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn redact_secret_like_text(value: &str) -> Result<String, MojoError> {
    let capacity = buffer_capacity(value.len())?;
    let mut scratch_one = vec![0_u8; capacity];
    let mut scratch_two = vec![0_u8; capacity];
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let result = unsafe {
        prodex_redaction_secret_like_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch_one.as_mut_ptr() as usize as u64,
            scratch_two.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    status(result)?;
    output_string(output, written)
}

fn redact_gateway_extra(value: &str) -> Result<String, MojoError> {
    let capacity = buffer_capacity(value.len())?;
    let mut scratch = vec![0_u8; capacity];
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let result = unsafe {
        prodex_redaction_gateway_extra_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    status(result)?;
    output_string(output, written)
}

pub fn redact_gateway_text(value: &str) -> Result<String, MojoError> {
    let redacted = redact_secret_like_text(value)?;
    redact_gateway_extra(&redacted)
}

/// One local detector result, encoded with the stable `FindingKind` ABI tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalInspectionMatch {
    pub start: usize,
    pub end: usize,
    pub kind: LocalInspectionFindingKind,
}

/// Mojo-redacted text and ordered byte ranges from the local inspection scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInspectionResult {
    pub text: String,
    pub matches: Vec<LocalInspectionMatch>,
}

/// Detects and masks local sensitive text. Mojo owns detector semantics,
/// match ordering, overlap resolution, and replacement; Rust validates ABI
/// records and owns only the returned storage.
pub fn local_inspect_and_redact(
    value: &str,
    sensitive_kind: Option<LocalInspectionFindingKind>,
    max_matches: usize,
) -> Result<LocalInspectionResult, MojoError> {
    let output_capacity = value
        .len()
        .checked_add(
            max_matches
                .checked_mul(REDACTION_LOCAL_PLACEHOLDER_EXPANSION)
                .ok_or(MojoError::Capacity)?,
        )
        .and_then(|capacity| capacity.checked_add(32))
        .ok_or(MojoError::Capacity)?;
    let match_storage_len = max_matches.checked_mul(3).ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; output_capacity];
    let mut match_storage = vec![-1_i64; match_storage_len];
    let mut match_count = 0_i64;
    let mut written = 0_i64;
    let result_status = unsafe {
        prodex_redaction_local_inspection_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            sensitive_kind.map_or(-1, |kind| kind as i64),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output_capacity).map_err(|_| MojoError::Capacity)?,
            match_storage.as_mut_ptr() as usize as u64,
            i64::try_from(max_matches).map_err(|_| MojoError::Capacity)?,
            (&mut match_count as *mut i64) as usize as u64,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    status(result_status)?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let match_count = usize::try_from(match_count).map_err(|_| MojoError::InvalidOutput)?;
    if match_count > max_matches {
        return Err(MojoError::InvalidOutput);
    }
    let mut matches = Vec::with_capacity(match_count);
    let mut covered_until = 0;
    for record in match_storage[..match_count * 3].as_chunks::<3>().0 {
        let start = usize::try_from(record[0]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(record[1]).map_err(|_| MojoError::InvalidOutput)?;
        let kind = LocalInspectionFindingKind::from_abi_tag(record[2])?;
        if start < covered_until || start >= end || end > value.len() {
            return Err(MojoError::InvalidOutput);
        }
        covered_until = end;
        matches.push(LocalInspectionMatch { start, end, kind });
    }
    Ok(LocalInspectionResult {
        text: String::from_utf8(output[..written].to_vec())
            .map_err(|_| MojoError::InvalidOutput)?,
        matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presidio_transport_policy_contract_is_mojo_authoritative() {
        assert!(!presidio_fail_closed(false, false, false, false, false).unwrap());
        assert!(presidio_fail_closed(true, false, false, false, false).unwrap());
        assert!(presidio_fail_closed(false, true, false, false, false).unwrap());
        assert!(presidio_fail_closed(false, false, true, false, false).unwrap());
        assert!(presidio_fail_closed(false, false, false, true, false).unwrap());
        assert!(presidio_fail_closed(false, false, false, false, true).unwrap());
        assert!(!presidio_local_inspection_required(true, false, false, false).unwrap());
        assert!(presidio_local_inspection_required(false, false, false, false).unwrap());
        assert!(presidio_external_coverage_denied(true, false).unwrap());
        assert!(!presidio_external_coverage_denied(true, true).unwrap());
        assert!(!presidio_external_coverage_denied(false, false).unwrap());
    }

    #[test]
    fn redaction_kernel_smoke() {
        assert!(key_looks_sensitive("X-API-Key").unwrap());
        assert_eq!(
            json_field_plan("arguments").unwrap(),
            JsonFieldPlan {
                inspection_mode: JsonInspectionMode::AllStrings,
                sensitive_kind: JsonSensitiveKind::None,
                known_protocol_metadata: false,
                unsupported_modality: false,
                tools_field: false,
            }
        );
        assert_eq!(
            json_field_plan("private_key").unwrap().sensitive_kind,
            JsonSensitiveKind::PrivateKey
        );
        assert!(json_field_plan("model").unwrap().known_protocol_metadata);
        assert!(json_field_plan("input_image").unwrap().unsupported_modality);
        assert!(json_field_plan("tools").unwrap().tools_field);
        assert_eq!(
            redact_secret_like_text("Authorization: Bearer synthetic-token").unwrap(),
            "Authorization: Bearer <redacted>"
        );
        assert_eq!(
            redact_gateway_text("user@example.test 4111-1111-1111-1111").unwrap(),
            "<redacted><redacted>"
        );
    }

    #[test]
    fn local_inspection_masks_supported_values_and_returns_byte_ranges() {
        let value = "héllo user@example.test | Bearer token-123 | sk-proj-1234567890 | card 4111-1111-1111-1111";
        let redaction = local_inspect_and_redact(value, None, 8).unwrap();

        assert_eq!(
            redaction.text,
            "héllo <redacted> | Bearer <redacted> | <redacted> | card <redacted>"
        );
        let range = |needle: &str| {
            let start = value.find(needle).unwrap();
            (start, start + needle.len())
        };
        let (email_start, email_end) = range("user@example.test");
        let (token_start, token_end) = range("token-123");
        let (api_key_start, api_key_end) = range("sk-proj-1234567890");
        let (account_start, account_end) = range("4111-1111-1111-1111");
        assert_eq!(
            redaction.matches,
            vec![
                LocalInspectionMatch {
                    start: email_start,
                    end: email_end,
                    kind: LocalInspectionFindingKind::EmailAddress
                },
                LocalInspectionMatch {
                    start: token_start,
                    end: token_end,
                    kind: LocalInspectionFindingKind::AccessToken
                },
                LocalInspectionMatch {
                    start: api_key_start,
                    end: api_key_end,
                    kind: LocalInspectionFindingKind::ApiKey
                },
                LocalInspectionMatch {
                    start: account_start,
                    end: account_end,
                    kind: LocalInspectionFindingKind::FinancialAccount
                },
            ]
        );
    }

    #[test]
    fn local_inspection_resolves_overlaps_and_sensitive_field_override() {
        let labeled = local_inspect_and_redact("api_key=sk-proj-1234567890", None, 8).unwrap();
        assert_eq!(labeled.text, "api_key=<redacted>");
        assert_eq!(
            labeled.matches,
            vec![LocalInspectionMatch {
                start: 8,
                end: 26,
                kind: LocalInspectionFindingKind::ApiKey,
            }]
        );

        let sensitive = local_inspect_and_redact(
            "tenant value",
            Some(LocalInspectionFindingKind::TenantSensitive),
            8,
        )
        .unwrap();
        assert_eq!(sensitive.text, "<redacted>");
        assert_eq!(
            sensitive.matches,
            vec![LocalInspectionMatch {
                start: 0,
                end: 12,
                kind: LocalInspectionFindingKind::TenantSensitive,
            }]
        );
        let empty =
            local_inspect_and_redact("", Some(LocalInspectionFindingKind::TenantSensitive), 8)
                .unwrap();
        assert_eq!(empty.text, "");
        assert!(empty.matches.is_empty());
    }

    #[test]
    fn local_inspection_rejects_more_matches_than_the_caller_capacity() {
        assert_eq!(
            local_inspect_and_redact("a@example.test b@example.test", None, 1),
            Err(MojoError::Capacity)
        );
        assert_eq!(
            local_inspect_and_redact("ordinary text", None, 0).unwrap(),
            LocalInspectionResult {
                text: "ordinary text".to_string(),
                matches: Vec::new(),
            }
        );
        assert_eq!(
            local_inspect_and_redact("a@example.test", None, 0),
            Err(MojoError::Capacity)
        );
    }
}
