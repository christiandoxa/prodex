const GEMINI_REQUEST_FIELD_PLAN_ABI_VERSION: i64 = 1;
const GEMINI_REQUEST_FIELD_PLAN_STATUS_ABI_MISMATCH: i64 = 1;
const GEMINI_REQUEST_FIELD_PLAN_STATUS_INVALID_INPUT: i64 = 2;
const GEMINI_REQUEST_FIELD_PLAN_STATUS_CAPACITY: i64 = 3;
const GEMINI_REQUEST_FIELD_SOURCE_COUNT: i64 = 34;

pub const GEMINI_REQUEST_FIELD_PLAN_MAX_FIELDS: usize = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiRequestFieldTarget {
    Temperature = 0,
    TopP = 1,
    MaxOutputTokens = 2,
    StopSequences = 3,
    TopK = 4,
    Seed = 5,
    PresencePenalty = 6,
    FrequencyPenalty = 7,
    ResponseMimeType = 8,
    ResponseSchema = 9,
    ResponseJsonSchema = 10,
    ResponseModalities = 11,
    MediaResolution = 12,
    AudioTimestamp = 13,
    SpeechConfig = 14,
    CandidateCount = 15,
    SafetySettings = 16,
    CachedContent = 17,
    Labels = 18,
}

impl TryFrom<i64> for GeminiRequestFieldTarget {
    type Error = crate::MojoError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Temperature,
            1 => Self::TopP,
            2 => Self::MaxOutputTokens,
            3 => Self::StopSequences,
            4 => Self::TopK,
            5 => Self::Seed,
            6 => Self::PresencePenalty,
            7 => Self::FrequencyPenalty,
            8 => Self::ResponseMimeType,
            9 => Self::ResponseSchema,
            10 => Self::ResponseJsonSchema,
            11 => Self::ResponseModalities,
            12 => Self::MediaResolution,
            13 => Self::AudioTimestamp,
            14 => Self::SpeechConfig,
            15 => Self::CandidateCount,
            16 => Self::SafetySettings,
            17 => Self::CachedContent,
            18 => Self::Labels,
            _ => return Err(crate::MojoError::InvalidOutput),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiRequestField {
    pub target: GeminiRequestFieldTarget,
    pub source_index: usize,
}

unsafe extern "C" {
    fn prodex_gemini_request_field_plan_v1(
        abi_version: i64,
        basic_fields: u64,
        extended_fields: u64,
        optional_fields: u64,
        output_targets: *mut i64,
        output_sources: *mut i64,
        output_capacity: i64,
        output_count: *mut i64,
    ) -> i64;
}

pub fn gemini_request_field_plan(
    basic_fields: u64,
    extended_fields: u64,
    optional_fields: u64,
) -> Result<Vec<GeminiRequestField>, crate::MojoError> {
    let mut targets = [0_i64; GEMINI_REQUEST_FIELD_PLAN_MAX_FIELDS];
    let mut sources = [0_i64; GEMINI_REQUEST_FIELD_PLAN_MAX_FIELDS];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_gemini_request_field_plan_v1(
            GEMINI_REQUEST_FIELD_PLAN_ABI_VERSION,
            basic_fields,
            extended_fields,
            optional_fields,
            targets.as_mut_ptr(),
            sources.as_mut_ptr(),
            i64::try_from(targets.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut output_count,
        )
    };
    match status {
        0 => {}
        GEMINI_REQUEST_FIELD_PLAN_STATUS_ABI_MISMATCH => {
            return Err(crate::MojoError::AbiMismatch);
        }
        GEMINI_REQUEST_FIELD_PLAN_STATUS_INVALID_INPUT => {
            return Err(crate::MojoError::InvalidInput);
        }
        GEMINI_REQUEST_FIELD_PLAN_STATUS_CAPACITY => {
            return Err(crate::MojoError::Capacity);
        }
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    let output_count =
        usize::try_from(output_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    if output_count > targets.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    targets[..output_count]
        .iter()
        .zip(&sources[..output_count])
        .map(|(&target, &source)| {
            Ok(GeminiRequestField {
                target: GeminiRequestFieldTarget::try_from(target)?,
                source_index: usize::try_from(source)
                    .ok()
                    .filter(|source| *source < GEMINI_REQUEST_FIELD_SOURCE_COUNT as usize)
                    .ok_or(crate::MojoError::InvalidOutput)?,
            })
        })
        .collect()
}
