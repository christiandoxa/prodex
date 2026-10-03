use super::{QuotaAdmissionValue, quota_model_policy_status};

const QUOTA_USAGE_RESPONSE_ABI_VERSION: i64 = 1;
const QUOTA_USAGE_RESPONSE_HEADER_FIELDS: usize = 10;
const QUOTA_USAGE_RESPONSE_ENTRY_FIELDS: usize = 10;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct QuotaUsageTextView {
    ptr: u64,
    len: u64,
}

/// CamelCase and snake_case forms of one string field from a quota payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaUsageStringAliases<'a> {
    pub snake_case: Option<&'a str>,
    pub camel_case: Option<&'a str>,
}

/// One entry from the provider's indexed rate-limit map.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaIndexedRateLimitInput<'a> {
    pub map_key: &'a str,
    pub limit_id: QuotaUsageStringAliases<'a>,
    pub plan_type: QuotaUsageStringAliases<'a>,
    pub limit_name: QuotaUsageStringAliases<'a>,
    pub metered_feature: QuotaUsageStringAliases<'a>,
}

/// Presence and type of both spelling variants of a quota admission field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaUsageAdmissionAliases {
    pub snake_case: QuotaAdmissionValue,
    pub camel_case: QuotaAdmissionValue,
}

/// Presence of preferred and legacy spellings for an opaque JSON metadata field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaUsagePresenceAliases {
    pub snake_case: bool,
    pub camel_case: bool,
}

/// Serde-decoded values needed to normalize an OpenAI usage response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaUsageResponseInput<'a> {
    pub indexed_rate_limits: &'a [QuotaIndexedRateLimitInput<'a>],
    pub existing_additional_limit_ids: &'a [Option<&'a str>],
    pub rate_limit: Option<QuotaUsageStringAliases<'a>>,
    pub rate_limits: Option<QuotaUsageStringAliases<'a>>,
    pub plan_type: Option<&'a str>,
    pub ordinary_usage_allowed: QuotaUsageAdmissionAliases,
    pub rate_limit_reached_type: QuotaAdmissionValue,
    pub rate_limit_upsell: QuotaUsagePresenceAliases,
    pub account_id: QuotaUsagePresenceAliases,
}

/// Source chosen for a normalized indexed text field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaUsageTextSource {
    None,
    MapKey,
    SnakeCase,
    CamelCase,
}

/// Source chosen for a metadata field copied from the top-level response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaUsageMetadataSource {
    None,
    SnakeCase,
    CamelCase,
}

/// Selected primary rate-limit pair in the decoded response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaUsageMainRateLimitSource {
    None,
    Indexed(usize),
    RateLimit,
    RateLimits,
}

/// A normalized string source and its UTF-8 byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaUsageTextSelection {
    pub source: QuotaUsageTextSource,
    pub start: usize,
    pub end: usize,
}

/// Mojo's normalization plan for one indexed rate-limit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaIndexedRateLimitPlan {
    pub include_as_additional: bool,
    pub limit_id: QuotaUsageTextSelection,
    pub limit_name: QuotaUsageTextSelection,
    pub metered_feature: QuotaUsageTextSelection,
}

/// Mojo's decisions for normalizing a decoded OpenAI usage response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaUsageResponsePlan {
    pub main_rate_limit: QuotaUsageMainRateLimitSource,
    pub force_main_rate_limit_denied: bool,
    pub preserve_ordinary_usage_allowed: bool,
    pub ordinary_usage_allowed_source: QuotaUsageMetadataSource,
    pub rate_limit_upsell_source: QuotaUsageMetadataSource,
    pub account_id_source: QuotaUsageMetadataSource,
    pub plan_type_from_input: bool,
    pub plan_type_source: QuotaUsageMetadataSource,
    pub plan_type_start: usize,
    pub plan_type_end: usize,
    pub indexed_rate_limits: Vec<QuotaIndexedRateLimitPlan>,
}

unsafe extern "C" {
    fn prodex_quota_usage_response_plan_v1(
        abi_version: i64,
        indexed_count: i64,
        indexed_keys: *const QuotaUsageTextView,
        indexed_limit_ids: *const QuotaUsageTextView,
        indexed_plan_types: *const QuotaUsageTextView,
        indexed_limit_names: *const QuotaUsageTextView,
        indexed_metered_features: *const QuotaUsageTextView,
        existing_count: i64,
        existing_limit_ids: *const QuotaUsageTextView,
        existing_id_present: *const i64,
        rate_limit_plan_types: *const QuotaUsageTextView,
        rate_limits_plan_types: *const QuotaUsageTextView,
        rate_limit_present: i64,
        rate_limits_present: i64,
        input_plan_type: QuotaUsageTextView,
        input_plan_type_present: i64,
        admission_kinds: *const i64,
        metadata_present: *const i64,
        output: *mut i64,
        output_capacity: i64,
    ) -> i64;
}

fn quota_usage_text_view(value: Option<&str>) -> Result<QuotaUsageTextView, crate::MojoError> {
    let Some(value) = value else {
        return Ok(QuotaUsageTextView::default());
    };
    let length = u64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    if i64::try_from(length).is_err() {
        return Err(crate::MojoError::InvalidInput);
    }
    Ok(QuotaUsageTextView {
        ptr: value.as_ptr() as usize as u64,
        len: length,
    })
}

fn quota_usage_append_aliases(
    output: &mut Vec<QuotaUsageTextView>,
    aliases: QuotaUsageStringAliases<'_>,
) -> Result<(), crate::MojoError> {
    output.push(quota_usage_text_view(aliases.snake_case)?);
    output.push(quota_usage_text_view(aliases.camel_case)?);
    Ok(())
}

fn quota_usage_admission_code(value: QuotaAdmissionValue) -> i64 {
    match value {
        QuotaAdmissionValue::Missing => 0,
        QuotaAdmissionValue::Null => 1,
        QuotaAdmissionValue::True => 2,
        QuotaAdmissionValue::False => 3,
        QuotaAdmissionValue::Other => 4,
    }
}

fn quota_usage_metadata_source(value: i64) -> Result<QuotaUsageMetadataSource, crate::MojoError> {
    match value {
        0 => Ok(QuotaUsageMetadataSource::None),
        1 => Ok(QuotaUsageMetadataSource::SnakeCase),
        2 => Ok(QuotaUsageMetadataSource::CamelCase),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

fn quota_usage_text_selection(
    output: &[i64],
    offset: usize,
    allow_map_key: bool,
) -> Result<QuotaUsageTextSelection, crate::MojoError> {
    let source = match output.get(offset).copied() {
        Some(0) => QuotaUsageTextSource::None,
        Some(1) if allow_map_key => QuotaUsageTextSource::MapKey,
        Some(2) => QuotaUsageTextSource::SnakeCase,
        Some(3) => QuotaUsageTextSource::CamelCase,
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    let start = output
        .get(offset + 1)
        .copied()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(crate::MojoError::InvalidOutput)?;
    let end = output
        .get(offset + 2)
        .copied()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(crate::MojoError::InvalidOutput)?;
    if start > end {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(QuotaUsageTextSelection { source, start, end })
}

/// Plans primary-bucket selection and indexed-bucket normalization in Mojo.
pub fn quota_usage_response_plan(
    input: QuotaUsageResponseInput<'_>,
) -> Result<QuotaUsageResponsePlan, crate::MojoError> {
    let indexed_count = input.indexed_rate_limits.len();
    let indexed_count_i64 =
        i64::try_from(indexed_count).map_err(|_| crate::MojoError::InvalidInput)?;
    let existing_count = input.existing_additional_limit_ids.len();
    let existing_count_i64 =
        i64::try_from(existing_count).map_err(|_| crate::MojoError::InvalidInput)?;
    let indexed_alias_count = indexed_count
        .checked_mul(2)
        .ok_or(crate::MojoError::InvalidInput)?;
    let output_length = indexed_count
        .checked_mul(QUOTA_USAGE_RESPONSE_ENTRY_FIELDS)
        .and_then(|length| length.checked_add(QUOTA_USAGE_RESPONSE_HEADER_FIELDS))
        .ok_or(crate::MojoError::InvalidInput)?;

    let mut indexed_keys = Vec::with_capacity(indexed_count);
    let mut indexed_limit_ids = Vec::with_capacity(indexed_alias_count);
    let mut indexed_plan_types = Vec::with_capacity(indexed_alias_count);
    let mut indexed_limit_names = Vec::with_capacity(indexed_alias_count);
    let mut indexed_metered_features = Vec::with_capacity(indexed_alias_count);
    for indexed in input.indexed_rate_limits {
        indexed_keys.push(quota_usage_text_view(Some(indexed.map_key))?);
        quota_usage_append_aliases(&mut indexed_limit_ids, indexed.limit_id)?;
        quota_usage_append_aliases(&mut indexed_plan_types, indexed.plan_type)?;
        quota_usage_append_aliases(&mut indexed_limit_names, indexed.limit_name)?;
        quota_usage_append_aliases(&mut indexed_metered_features, indexed.metered_feature)?;
    }

    let mut existing_limit_ids = Vec::with_capacity(existing_count);
    let mut existing_id_present = Vec::with_capacity(existing_count);
    for limit_id in input.existing_additional_limit_ids {
        existing_limit_ids.push(quota_usage_text_view(*limit_id)?);
        existing_id_present.push(i64::from(limit_id.is_some()));
    }
    let rate_limit_plan_types = input.rate_limit.unwrap_or_default();
    let rate_limit_plan_types = [
        quota_usage_text_view(rate_limit_plan_types.snake_case)?,
        quota_usage_text_view(rate_limit_plan_types.camel_case)?,
    ];
    let rate_limits_plan_types = input.rate_limits.unwrap_or_default();
    let rate_limits_plan_types = [
        quota_usage_text_view(rate_limits_plan_types.snake_case)?,
        quota_usage_text_view(rate_limits_plan_types.camel_case)?,
    ];
    let input_plan_type = quota_usage_text_view(input.plan_type)?;
    let admission_kinds = [
        quota_usage_admission_code(input.ordinary_usage_allowed.snake_case),
        quota_usage_admission_code(input.ordinary_usage_allowed.camel_case),
        quota_usage_admission_code(input.rate_limit_reached_type),
    ];
    let metadata_present = [
        i64::from(input.rate_limit_upsell.snake_case),
        i64::from(input.rate_limit_upsell.camel_case),
        i64::from(input.account_id.snake_case),
        i64::from(input.account_id.camel_case),
    ];
    let mut output = vec![0_i64; output_length];
    let status = unsafe {
        prodex_quota_usage_response_plan_v1(
            QUOTA_USAGE_RESPONSE_ABI_VERSION,
            indexed_count_i64,
            indexed_keys.as_ptr(),
            indexed_limit_ids.as_ptr(),
            indexed_plan_types.as_ptr(),
            indexed_limit_names.as_ptr(),
            indexed_metered_features.as_ptr(),
            existing_count_i64,
            existing_limit_ids.as_ptr(),
            existing_id_present.as_ptr(),
            rate_limit_plan_types.as_ptr(),
            rate_limits_plan_types.as_ptr(),
            i64::from(input.rate_limit.is_some()),
            i64::from(input.rate_limits.is_some()),
            input_plan_type,
            i64::from(input.plan_type.is_some()),
            admission_kinds.as_ptr(),
            metadata_present.as_ptr(),
            output.as_mut_ptr(),
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    quota_model_policy_status(status)?;

    let main_rate_limit = match output[0] {
        -1 => QuotaUsageMainRateLimitSource::None,
        source if usize::try_from(source).is_ok_and(|index| index < indexed_count) => {
            QuotaUsageMainRateLimitSource::Indexed(
                usize::try_from(source).map_err(|_| crate::MojoError::InvalidOutput)?,
            )
        }
        source if source == indexed_count_i64 && input.rate_limit.is_some() => {
            QuotaUsageMainRateLimitSource::RateLimit
        }
        source if source == indexed_count_i64 + 1 && input.rate_limits.is_some() => {
            QuotaUsageMainRateLimitSource::RateLimits
        }
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    let bool_field = |index: usize| match output.get(index).copied() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    };
    let force_main_rate_limit_denied = bool_field(1)?;
    let preserve_ordinary_usage_allowed = bool_field(2)?;
    let ordinary_usage_allowed_source = quota_usage_metadata_source(output[3])?;
    let rate_limit_upsell_source = quota_usage_metadata_source(output[4])?;
    let account_id_source = quota_usage_metadata_source(output[5])?;
    let plan_type_from_input = bool_field(6)?;
    if plan_type_from_input != input.plan_type.is_some() {
        return Err(crate::MojoError::InvalidOutput);
    }
    let plan_type_source = quota_usage_metadata_source(output[7])?;
    let plan_type_start =
        usize::try_from(output[8]).map_err(|_| crate::MojoError::InvalidOutput)?;
    let plan_type_end = usize::try_from(output[9]).map_err(|_| crate::MojoError::InvalidOutput)?;
    if plan_type_start > plan_type_end
        || (plan_type_from_input && plan_type_source != QuotaUsageMetadataSource::None)
        || (!plan_type_from_input
            && main_rate_limit == QuotaUsageMainRateLimitSource::None
            && plan_type_source != QuotaUsageMetadataSource::None)
    {
        return Err(crate::MojoError::InvalidOutput);
    }

    let mut indexed_rate_limits = Vec::with_capacity(indexed_count);
    for index in 0..indexed_count {
        let offset = QUOTA_USAGE_RESPONSE_HEADER_FIELDS + index * QUOTA_USAGE_RESPONSE_ENTRY_FIELDS;
        let include_as_additional = match output.get(offset).copied() {
            Some(0) => false,
            Some(1) => true,
            _ => return Err(crate::MojoError::InvalidOutput),
        };
        indexed_rate_limits.push(QuotaIndexedRateLimitPlan {
            include_as_additional,
            limit_id: quota_usage_text_selection(&output, offset + 1, true)?,
            limit_name: quota_usage_text_selection(&output, offset + 4, false)?,
            metered_feature: quota_usage_text_selection(&output, offset + 7, false)?,
        });
    }

    Ok(QuotaUsageResponsePlan {
        main_rate_limit,
        force_main_rate_limit_denied,
        preserve_ordinary_usage_allowed,
        ordinary_usage_allowed_source,
        rate_limit_upsell_source,
        account_id_source,
        plan_type_from_input,
        plan_type_source,
        plan_type_start,
        plan_type_end,
        indexed_rate_limits,
    })
}
