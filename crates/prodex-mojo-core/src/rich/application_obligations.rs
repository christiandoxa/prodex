use super::*;

pub const APPLICATION_OBLIGATION_ABI_VERSION: i64 = 1;
pub const APPLICATION_OBLIGATION_MAX_COUNT: usize = 256;
pub const APPLICATION_OBLIGATION_MAX_VIOLATIONS: usize = 32;

pub const APPLICATION_OBLIGATION_MASK_FINDING: u8 = 0;
pub const APPLICATION_OBLIGATION_DISABLE_TOOLS: u8 = 1;
pub const APPLICATION_OBLIGATION_ALLOW_TOOL: u8 = 2;
pub const APPLICATION_OBLIGATION_ALLOW_MODEL: u8 = 3;
pub const APPLICATION_OBLIGATION_ALLOW_MODALITY: u8 = 4;
pub const APPLICATION_OBLIGATION_MAX_INPUT_TOKENS: u8 = 5;
pub const APPLICATION_OBLIGATION_MAX_OUTPUT_TOKENS: u8 = 6;
pub const APPLICATION_OBLIGATION_MAX_CONTEXT_TOKENS: u8 = 7;
pub const APPLICATION_OBLIGATION_REQUIRE_RESPONSE_INSPECTION: u8 = 8;
pub const APPLICATION_OBLIGATION_SESSION_IDLE_TIMEOUT: u8 = 9;
pub const APPLICATION_OBLIGATION_SESSION_ABSOLUTE_TIMEOUT: u8 = 10;
pub const APPLICATION_OBLIGATION_MIN_AUTHENTICATION_STRENGTH: u8 = 11;
pub const APPLICATION_OBLIGATION_REQUIRE_REAUTHENTICATION: u8 = 12;
pub const APPLICATION_OBLIGATION_REQUIRE_MFA: u8 = 13;
pub const APPLICATION_OBLIGATION_REQUIRE_HUMAN_APPROVAL: u8 = 14;
pub const APPLICATION_OBLIGATION_OTHER: u8 = 15;

/// Normalized, borrowed obligation record. Domain-specific Rust types are
/// converted to these tags before crossing the ABI.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationObligationRecord<'a> {
    pub kind: u8,
    pub value: u64,
    pub selector: Option<&'a str>,
}

/// Scalar and borrowed text inputs for application obligation planning.
#[derive(Clone, Copy)]
pub struct ApplicationObligationInput<'a> {
    pub mode: u8,
    pub effect: u8,
    pub obligations: &'a [ApplicationObligationRecord<'a>],
    pub classification: u8,
    pub inspection_coverage: u8,
    pub detected_findings_mask: u16,
    pub masked_findings_mask: u16,
    pub requested_capabilities_mask: u8,
    pub requested_model: Option<&'a str>,
    pub requested_tools: Option<&'a [&'a str]>,
    pub requested_modalities_mask: u8,
    pub estimated_input_tokens: u32,
    pub estimated_context_tokens: u32,
    pub requested_output_tokens: Option<u32>,
    pub session_age_seconds: u64,
    pub session_idle_seconds: u64,
    pub session_revoked: bool,
    pub session_mfa_satisfied: bool,
    pub authentication_strength: u8,
    pub environment_mfa_satisfied: bool,
    pub reauthentication_satisfied: bool,
    pub response_transport: u8,
    pub response_inspection_coverage: u8,
}

/// Mojo's deterministic application obligation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationObligationPlan {
    pub mask_findings: Vec<u8>,
    pub disable_tools: bool,
    pub maximum_input_tokens: Option<u32>,
    pub maximum_context_tokens: Option<u32>,
    pub maximum_output_tokens: Option<u32>,
    pub enforce: bool,
    pub inspection_required: bool,
    pub require_full_inspection: bool,
    pub violation_tags: Vec<u8>,
    pub disposition: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationObligationResult {
    abi_version: i64,
    mask_count: i64,
    disable_tools: i64,
    maximum_input_present: i64,
    maximum_input_tokens: u64,
    maximum_context_present: i64,
    maximum_context_tokens: u64,
    maximum_output_present: i64,
    maximum_output_tokens: u64,
    enforce: i64,
    inspection_required: i64,
    require_full_inspection: i64,
    violation_count: i64,
    disposition: i64,
}

const _: () = assert!(std::mem::size_of::<ApplicationObligationResult>() == 112);

unsafe extern "C" {
    fn prodex_mojo_rich_application_obligation_plan_v1(
        abi_version: i64,
        mode: i64,
        effect: i64,
        obligations: u64,
        obligation_values: u64,
        obligation_selectors: u64,
        obligation_count: i64,
        classification: i64,
        inspection_coverage: i64,
        detected_findings_mask: i64,
        masked_findings_mask: i64,
        requested_capabilities_mask: i64,
        requested_model: u64,
        requested_model_present: i64,
        requested_tools: u64,
        requested_tools_present: i64,
        requested_tools_count: i64,
        requested_modalities_mask: i64,
        estimated_input_tokens: u64,
        estimated_context_tokens: u64,
        requested_output_present: i64,
        requested_output_tokens: u64,
        session_age_seconds: u64,
        session_idle_seconds: u64,
        session_revoked: i64,
        session_mfa_satisfied: i64,
        authentication_strength: i64,
        environment_mfa_satisfied: i64,
        reauthentication_satisfied: i64,
        response_transport: i64,
        response_inspection_coverage: i64,
        output_masks: u64,
        output_mask_capacity: i64,
        output_violations: u64,
        output_violation_capacity: i64,
        result: u64,
    ) -> i64;
}

/// Run Mojo-owned obligation planning over bounded caller-owned views.
pub fn plan_application_obligations(
    input: ApplicationObligationInput<'_>,
) -> Result<ApplicationObligationPlan, MojoError> {
    ensure_rich_abi()?;
    if input.obligations.len() > APPLICATION_OBLIGATION_MAX_COUNT
        || input
            .requested_tools
            .is_some_and(|tools| tools.len() > APPLICATION_OBLIGATION_MAX_COUNT)
    {
        return Err(MojoError::InvalidInput);
    }

    let kinds = input
        .obligations
        .iter()
        .map(|obligation| i64::from(obligation.kind))
        .collect::<Vec<_>>();
    let values = input
        .obligations
        .iter()
        .map(|obligation| obligation.value)
        .collect::<Vec<_>>();
    let selectors = input
        .obligations
        .iter()
        .map(|obligation| obligation.selector.map(view).unwrap_or_default())
        .collect::<Vec<_>>();
    let requested_model = input.requested_model.map(view).unwrap_or_default();
    let requested_tools = input
        .requested_tools
        .map(|tools| tools.iter().map(|tool| view(tool)).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut mask_findings = vec![0_i64; input.obligations.len().max(1)];
    let mut violation_tags = vec![0_i64; APPLICATION_OBLIGATION_MAX_VIOLATIONS];
    let mut result = ApplicationObligationResult::default();
    let status = unsafe {
        prodex_mojo_rich_application_obligation_plan_v1(
            APPLICATION_OBLIGATION_ABI_VERSION,
            i64::from(input.mode),
            i64::from(input.effect),
            mojo_pointer_address(kinds.as_ptr()),
            mojo_pointer_address(values.as_ptr()),
            mojo_pointer_address(selectors.as_ptr()),
            i64::try_from(input.obligations.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.classification),
            i64::from(input.inspection_coverage),
            i64::from(input.detected_findings_mask),
            i64::from(input.masked_findings_mask),
            i64::from(input.requested_capabilities_mask),
            mojo_pointer_address(&requested_model),
            i64::from(input.requested_model.is_some()),
            mojo_pointer_address(requested_tools.as_ptr()),
            i64::from(input.requested_tools.is_some()),
            i64::try_from(requested_tools.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.requested_modalities_mask),
            u64::from(input.estimated_input_tokens),
            u64::from(input.estimated_context_tokens),
            i64::from(input.requested_output_tokens.is_some()),
            u64::from(input.requested_output_tokens.unwrap_or_default()),
            input.session_age_seconds,
            input.session_idle_seconds,
            i64::from(input.session_revoked),
            i64::from(input.session_mfa_satisfied),
            i64::from(input.authentication_strength),
            i64::from(input.environment_mfa_satisfied),
            i64::from(input.reauthentication_satisfied),
            i64::from(input.response_transport),
            i64::from(input.response_inspection_coverage),
            mojo_mut_pointer_address(mask_findings.as_mut_ptr()),
            i64::try_from(mask_findings.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(violation_tags.as_mut_ptr()),
            i64::try_from(violation_tags.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 9, 0, 0, 0));
    }
    if result.abi_version != APPLICATION_OBLIGATION_ABI_VERSION
        || result.mask_count < 0
        || result.mask_count as usize > mask_findings.len()
        || result.mask_count as usize > input.obligations.len()
        || result.violation_count < 0
        || result.violation_count as usize > violation_tags.len()
    {
        return Err(MojoError::InvalidOutput);
    }

    let mask_findings = decode_tags(&mask_findings, result.mask_count, 12)?;
    let violation_tags = decode_tags(&violation_tags, result.violation_count, 19)?;
    Ok(ApplicationObligationPlan {
        mask_findings,
        disable_tools: decode_flag(result.disable_tools)?,
        maximum_input_tokens: decode_optional_u32(
            result.maximum_input_present,
            result.maximum_input_tokens,
        )?,
        maximum_context_tokens: decode_optional_u32(
            result.maximum_context_present,
            result.maximum_context_tokens,
        )?,
        maximum_output_tokens: decode_optional_u32(
            result.maximum_output_present,
            result.maximum_output_tokens,
        )?,
        enforce: decode_flag(result.enforce)?,
        inspection_required: decode_flag(result.inspection_required)?,
        require_full_inspection: decode_flag(result.require_full_inspection)?,
        violation_tags,
        disposition: match result.disposition {
            0 | 1 => result.disposition as u8,
            _ => return Err(MojoError::InvalidOutput),
        },
    })
}

fn decode_flag(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_optional_u32(present: i64, value: u64) -> Result<Option<u32>, MojoError> {
    match decode_flag(present)? {
        true => u32::try_from(value)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        false if value == 0 => Ok(None),
        false => Err(MojoError::InvalidOutput),
    }
}

fn decode_tags(values: &[i64], count: i64, maximum: i64) -> Result<Vec<u8>, MojoError> {
    let values = &values[..usize::try_from(count).map_err(|_| MojoError::InvalidOutput)?];
    let mut decoded = Vec::with_capacity(values.len());
    for value in values {
        if *value < 0 || *value >= maximum {
            return Err(MojoError::InvalidOutput);
        }
        decoded.push(u8::try_from(*value).map_err(|_| MojoError::InvalidOutput)?);
    }
    if decoded.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(decoded)
}
