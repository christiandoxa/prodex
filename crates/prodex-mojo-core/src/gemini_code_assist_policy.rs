use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_gemini_oauth_binding_priority_v1(
        abi_version: i64,
        previous_bound: i64,
        tool_bound_address: u64,
        tool_count: i64,
        selected_address: u64,
    ) -> i64;
    fn prodex_gemini_oauth_profile_rotation_v1(
        abi_version: i64,
        next_index: u64,
        eligible_address: u64,
        profile_count: i64,
        indices_address: u64,
        index_capacity: i64,
        written_address: u64,
        next_index_address: u64,
    ) -> i64;
    fn prodex_gemini_oauth_model_availability_v1(
        abi_version: i64,
        now_ms: u64,
        cooldown_until_address: u64,
        count: i64,
        mask_address: u64,
        mask_capacity: i64,
        any_available_address: u64,
    ) -> i64;
    fn prodex_gemini_oauth_model_preference_v1(
        abi_version: i64,
        name_views_address: u64,
        model_count: i64,
        preference_address: u64,
        preference_length: i64,
        preference_present: i64,
        indices_address: u64,
        index_capacity: i64,
        written_count_address: u64,
    ) -> i64;

    fn prodex_gemini_code_assist_endpoint_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;

    fn prodex_gemini_code_assist_tier_label_v1(
        abi_version: i64,
        id_address: u64,
        id_length: i64,
        id_present: i64,
        name_address: u64,
        name_length: i64,
        name_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_optional_string(
    output: Vec<u8>,
    written: i64,
    present: i64,
) -> Result<Option<String>, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    match present {
        0 if written == 0 => Ok(None),
        1 => String::from_utf8(output[..written].to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn normalize_gemini_code_assist_endpoint(input: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; input.len().max(1)];
    let mut written = 0_i64;
    let mut present = 0_i64;
    status(unsafe {
        prodex_gemini_code_assist_endpoint_v1(
            ABI_VERSION,
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    decode_optional_string(output, written, present)
}

pub fn gemini_code_assist_tier_label(
    id: Option<&str>,
    name: Option<&str>,
) -> Result<Option<String>, MojoError> {
    let id_value = id.unwrap_or_default();
    let name_value = name.unwrap_or_default();
    let capacity = id_value
        .len()
        .max(name_value.len())
        .max("standard".len())
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let mut present = 0_i64;
    status(unsafe {
        prodex_gemini_code_assist_tier_label_v1(
            ABI_VERSION,
            id_value.as_ptr() as usize as u64,
            signed(id_value.len())?,
            i64::from(id.is_some()),
            name_value.as_ptr() as usize as u64,
            signed(name_value.len())?,
            i64::from(name.is_some()),
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    decode_optional_string(output, written, present)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiOAuthBindingChoice {
    PreviousResponse,
    ToolOutput(usize),
}

/// The Mojo owner prioritizes a bound previous_response_id before the first
/// bound tool-output call, identically for profile and model-scope affinity.
pub fn gemini_oauth_binding_priority(
    previous_bound: bool,
    tool_bound: &[bool],
) -> Result<Option<GeminiOAuthBindingChoice>, MojoError> {
    let bits = tool_bound
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let mut selected = i64::MIN;
    status(unsafe {
        prodex_gemini_oauth_binding_priority_v1(
            ABI_VERSION,
            i64::from(previous_bound),
            bits.as_ptr() as usize as u64,
            signed(bits.len())?,
            (&mut selected as *mut i64) as usize as u64,
        )
    })?;
    match selected {
        -2 => Ok(None),
        -1 if previous_bound => Ok(Some(GeminiOAuthBindingChoice::PreviousResponse)),
        -1 => Err(MojoError::InvalidOutput),
        index if index >= 0 => {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            match tool_bound.get(index) {
                Some(true) => Ok(Some(GeminiOAuthBindingChoice::ToolOutput(index))),
                _ => Err(MojoError::InvalidOutput),
            }
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiOAuthProfileRotationPlan {
    pub indices: Vec<usize>,
    pub next_index: usize,
}

/// Compiled Mojo alone chooses round-robin start, eligible profile order,
/// last-chance all-profile fallback and next cursor. Affinity is resolved
/// separately before invoking this fresh-request rotation plan.
pub fn gemini_oauth_profile_rotation_plan(
    next_index: usize,
    eligible: &[bool],
) -> Result<GeminiOAuthProfileRotationPlan, MojoError> {
    if eligible.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let input = eligible
        .iter()
        .map(|flag| i64::from(*flag))
        .collect::<Vec<_>>();
    let mut indices = vec![u64::MAX; eligible.len()];
    let mut written = -1_i64;
    let mut next = u64::MAX;
    status(unsafe {
        prodex_gemini_oauth_profile_rotation_v1(
            ABI_VERSION,
            u64::try_from(next_index).map_err(|_| MojoError::InvalidInput)?,
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            indices.as_mut_ptr() as usize as u64,
            signed(indices.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut next as *mut u64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let next_index = usize::try_from(next).map_err(|_| MojoError::InvalidOutput)?;
    if written == 0 || written > eligible.len() || next_index >= eligible.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; eligible.len()];
    let mut selected = Vec::with_capacity(written);
    for &index in &indices[..written] {
        let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        let entry = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *entry {
            return Err(MojoError::InvalidOutput);
        }
        *entry = true;
        selected.push(index);
    }
    Ok(GeminiOAuthProfileRotationPlan {
        indices: selected,
        next_index,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiOAuthModelDeadline {
    pub cooldown_until_ms: u64,
    pub unavailable_until_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiOAuthModelAvailability {
    pub eligible: Vec<bool>,
    pub any_available: bool,
}

/// Compiled Mojo decides endpoint cooldown and model unavailability for each
/// model. The caller supplies timestamps from the Rust-owned state map.
pub fn gemini_oauth_model_availability(
    deadlines: &[GeminiOAuthModelDeadline],
    now_ms: u64,
) -> Result<GeminiOAuthModelAvailability, MojoError> {
    let capacity = deadlines.len().checked_mul(2).ok_or(MojoError::Capacity)?;
    let mut raw = Vec::with_capacity(capacity);
    for deadline in deadlines {
        raw.extend([deadline.cooldown_until_ms, deadline.unavailable_until_ms]);
    }
    let mut mask = vec![-1_i64; deadlines.len()];
    let mut any = -1_i64;
    status(unsafe {
        prodex_gemini_oauth_model_availability_v1(
            ABI_VERSION,
            now_ms,
            raw.as_ptr() as usize as u64,
            signed(deadlines.len())?,
            mask.as_mut_ptr() as usize as u64,
            signed(mask.len())?,
            (&mut any as *mut i64) as usize as u64,
        )
    })?;
    let eligible = mask
        .into_iter()
        .map(|item| match item {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let any_available = match any {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(GeminiOAuthModelAvailability {
        eligible,
        any_available,
    })
}

/// Compiled Mojo chooses the preferred model's original index, if present,
/// then retains all nonpreferred models in original stable order. Multiple
/// copies of the preferred model collapse to one, preserving the old policy.
pub fn gemini_oauth_preferred_model_chain(
    models: &[String],
    preference: Option<&str>,
) -> Result<Vec<String>, MojoError> {
    let mut views = Vec::with_capacity(models.len().checked_mul(2).ok_or(MojoError::Capacity)?);
    for model in models {
        views.extend([
            model.as_ptr() as usize as u64,
            u64::try_from(model.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let preference_value = preference.unwrap_or("");
    let mut indices = vec![u64::MAX; models.len()];
    let mut written = -1_i64;
    status(unsafe {
        prodex_gemini_oauth_model_preference_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            signed(models.len())?,
            preference_value.as_ptr() as usize as u64,
            signed(preference_value.len())?,
            i64::from(preference.is_some()),
            indices.as_mut_ptr() as usize as u64,
            signed(indices.len())?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > models.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; models.len()];
    let mut reordered = Vec::with_capacity(count);
    for &index in &indices[..count] {
        let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        let item = models.get(index).ok_or(MojoError::InvalidOutput)?;
        if seen[index] {
            return Err(MojoError::InvalidOutput);
        }
        seen[index] = true;
        reordered.push(item.clone());
    }
    Ok(reordered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_oauth_compiled_binding_priority_preserves_response_then_first_tool() {
        use GeminiOAuthBindingChoice::*;
        for (previous, tools, expected) in [
            (false, vec![], None),
            (true, vec![], Some(PreviousResponse)),
            (false, vec![false], None),
            (true, vec![false], Some(PreviousResponse)),
            (false, vec![false, true, true], Some(ToolOutput(1))),
            (false, vec![true, false, true], Some(ToolOutput(0))),
            (true, vec![false, true, true], Some(PreviousResponse)),
            (false, vec![false, false, true], Some(ToolOutput(2))),
        ] {
            assert_eq!(
                gemini_oauth_binding_priority(previous, &tools).unwrap(),
                expected,
                "previous={previous} tool={tools:?}",
            );
        }
    }

    #[test]
    fn gemini_oauth_compiled_binding_priority_abi_rejects_bad_shape_without_writes() {
        let bits = [0_i64, 2_i64];
        let bits_ptr = bits.as_ptr() as usize as u64;
        let mut output = 37_i64;
        let dst = (&mut output as *mut i64) as usize as u64;
        let call = |version, previous, address, count, out| unsafe {
            prodex_gemini_oauth_binding_priority_v1(version, previous, address, count, out)
        };
        assert_eq!(call(2, 1, bits_ptr, 2, dst), 4);
        assert_eq!(call(1, 2, bits_ptr, 2, dst), 1);
        assert_eq!(call(1, 1, bits_ptr, -1, dst), 1);
        assert_eq!(call(1, 1, 0, 2, dst), 1);
        assert_eq!(call(1, 1, bits_ptr, 2, 0), 1);
        assert_eq!(call(1, 1, bits_ptr, 2, dst), 1);
        assert_eq!(output, 37);
    }

    #[test]
    fn gemini_oauth_compiled_profile_rotation_preserves_fairness_filter_and_last_chance() {
        for (next, eligible, expected, expected_next) in [
            (0, vec![true, true, true], vec![0, 1, 2], 1),
            (1, vec![true, true, true], vec![1, 2, 0], 2),
            (2, vec![true, true, true], vec![2, 0, 1], 0),
            (50, vec![true, true, true], vec![2, 0, 1], 0),
            (2, vec![false, true, false], vec![1], 0),
            (0, vec![false, true, true], vec![1, 2], 1),
            (1, vec![false, false, false], vec![1, 2, 0], 2),
            (0, vec![false], vec![0], 0),
            (0, vec![true], vec![0], 0),
        ] {
            assert_eq!(
                gemini_oauth_profile_rotation_plan(next, &eligible).unwrap(),
                GeminiOAuthProfileRotationPlan {
                    indices: expected,
                    next_index: expected_next,
                },
                "next={next} eligible={eligible:?}",
            );
        }
        assert_eq!(
            gemini_oauth_profile_rotation_plan(0, &[]),
            Err(MojoError::InvalidInput),
        );
    }

    #[test]
    fn gemini_oauth_compiled_profile_rotation_abi_rejects_invalid_mask_before_writes() {
        let eligible = [1_i64, 0, 2];
        let input = eligible.as_ptr() as usize as u64;
        let mut indices = [91_u64; 3];
        let mut written = 89_i64;
        let mut next = 73_u64;
        let out = indices.as_mut_ptr() as usize as u64;
        let count_ptr = (&mut written as *mut i64) as usize as u64;
        let next_ptr = (&mut next as *mut u64) as usize as u64;
        let call = |version, data, count, output, cap, written_ptr, next_ptr| unsafe {
            prodex_gemini_oauth_profile_rotation_v1(
                version,
                1,
                data,
                count,
                output,
                cap,
                written_ptr,
                next_ptr,
            )
        };
        assert_eq!(call(2, input, 3, out, 3, count_ptr, next_ptr), 4);
        assert_eq!(call(1, input, 0, out, 3, count_ptr, next_ptr), 1);
        assert_eq!(call(1, 0, 3, out, 3, count_ptr, next_ptr), 1);
        assert_eq!(call(1, input, 3, 0, 3, count_ptr, next_ptr), 1);
        assert_eq!(call(1, input, 3, out, 2, count_ptr, next_ptr), 1);
        assert_eq!(call(1, input, 3, out, 3, 0, next_ptr), 1);
        assert_eq!(call(1, input, 3, out, 3, count_ptr, 0), 1);
        assert_eq!(call(1, input, 3, out, 3, count_ptr, next_ptr), 1);
        assert_eq!(indices, [91; 3]);
        assert_eq!(written, 89);
        assert_eq!(next, 73);
    }

    #[test]
    fn gemini_oauth_compiled_availability_enforces_both_deadlines_and_clock_boundaries() {
        let deadlines = [
            GeminiOAuthModelDeadline {
                cooldown_until_ms: 0,
                unavailable_until_ms: 0,
            },
            GeminiOAuthModelDeadline {
                cooldown_until_ms: 100,
                unavailable_until_ms: 110,
            },
            GeminiOAuthModelDeadline {
                cooldown_until_ms: 111,
                unavailable_until_ms: 0,
            },
            GeminiOAuthModelDeadline {
                cooldown_until_ms: 0,
                unavailable_until_ms: 111,
            },
        ];
        assert_eq!(
            gemini_oauth_model_availability(&deadlines, 110).unwrap(),
            GeminiOAuthModelAvailability {
                eligible: vec![true, true, false, false],
                any_available: true,
            },
        );
        assert_eq!(
            gemini_oauth_model_availability(&deadlines[2..], 100).unwrap(),
            GeminiOAuthModelAvailability {
                eligible: vec![false, false],
                any_available: false,
            },
        );
        assert_eq!(
            gemini_oauth_model_availability(&[], 100).unwrap(),
            GeminiOAuthModelAvailability {
                eligible: vec![],
                any_available: false
            },
        );
        assert_eq!(
            gemini_oauth_model_availability(&deadlines, u64::MAX)
                .unwrap()
                .eligible,
            [true, true, true, true],
        );
    }

    #[test]
    fn gemini_oauth_compiled_preference_moves_existing_model_once_preserving_stable_order() {
        let models = vec![
            "gemini-3-pro-preview".to_string(),
            "東京🔥".to_string(),
            "gemini-2.5-flash".to_string(),
            "東京🔥".to_string(),
            "other".to_string(),
        ];
        assert_eq!(
            gemini_oauth_preferred_model_chain(&models, None).unwrap(),
            models
        );
        assert_eq!(
            gemini_oauth_preferred_model_chain(&models, Some("unknown")).unwrap(),
            models,
        );
        assert_eq!(
            gemini_oauth_preferred_model_chain(&models, Some("東京🔥")).unwrap(),
            [
                "東京🔥",
                "gemini-3-pro-preview",
                "gemini-2.5-flash",
                "other"
            ],
        );
        assert_eq!(
            gemini_oauth_preferred_model_chain(&models, Some("GEMINI-2.5-FLASH")).unwrap(),
            models,
        );
        assert_eq!(
            gemini_oauth_preferred_model_chain(&[], Some("abc")).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            gemini_oauth_preferred_model_chain(&["".to_string(), "model".to_string()], Some(""))
                .unwrap(),
            ["".to_string(), "model".to_string()],
        );
    }

    #[test]
    fn gemini_oauth_compiled_model_policy_abi_rejects_invalid_inputs_without_mutation() {
        let pair = [101_u64, 121_u64];
        let deadline_addr = pair.as_ptr() as usize as u64;
        let mut mask = [47_i64; 2];
        let mut any = 97_i64;
        let out = mask.as_mut_ptr() as usize as u64;
        let any_ptr = (&mut any as *mut i64) as usize as u64;
        let availability = |version, data, count, output, cap, any_result| unsafe {
            prodex_gemini_oauth_model_availability_v1(
                version, 120, data, count, output, cap, any_result,
            )
        };
        assert_eq!(availability(2, deadline_addr, 1, out, 2, any_ptr), 4);
        assert_eq!(availability(1, 0, 1, out, 2, any_ptr), 1);
        assert_eq!(availability(1, deadline_addr, -1, out, 2, any_ptr), 1);
        assert_eq!(availability(1, deadline_addr, 1, 0, 2, any_ptr), 1);
        assert_eq!(availability(1, deadline_addr, 1, out, 0, any_ptr), 1);
        assert_eq!(availability(1, deadline_addr, 1, out, 2, 0), 1);
        assert_eq!(mask, [47; 2]);
        assert_eq!(any, 97);
        let model = b"test";
        let bad = [0xff_u8];
        let views = [model.as_ptr() as usize as u64, 4_u64];
        let bad_views = [bad.as_ptr() as usize as u64, 1_u64];
        let mut indices = [71_u64; 2];
        let mut written = 83_i64;
        let order_ptr = indices.as_mut_ptr() as usize as u64;
        let written_ptr = (&mut written as *mut i64) as usize as u64;
        let preference = |version, names, count, present, output, cap| unsafe {
            prodex_gemini_oauth_model_preference_v1(
                version,
                names,
                count,
                model.as_ptr() as usize as u64,
                4,
                present,
                output,
                cap,
                written_ptr,
            )
        };
        assert_eq!(
            preference(2, views.as_ptr() as usize as u64, 1, 1, order_ptr, 2),
            4
        );
        assert_eq!(preference(1, 0, 1, 1, order_ptr, 2), 1);
        assert_eq!(
            preference(1, views.as_ptr() as usize as u64, -1, 1, order_ptr, 2),
            1
        );
        assert_eq!(
            preference(1, views.as_ptr() as usize as u64, 1, 2, order_ptr, 2),
            1
        );
        assert_eq!(preference(1, views.as_ptr() as usize as u64, 1, 1, 0, 2), 1);
        assert_eq!(
            preference(1, views.as_ptr() as usize as u64, 1, 1, order_ptr, 0),
            1
        );
        assert_eq!(
            preference(1, bad_views.as_ptr() as usize as u64, 1, 1, order_ptr, 2),
            1
        );
        assert_eq!(indices, [71; 2]);
        assert_eq!(written, 83);
    }

    #[test]
    fn code_assist_policy_preserves_endpoint_normalization() {
        assert_eq!(
            normalize_gemini_code_assist_endpoint(" https://example.test/v1/// ").unwrap(),
            Some("https://example.test/v1".to_string())
        );
        assert_eq!(
            normalize_gemini_code_assist_endpoint(" /// ").unwrap(),
            None
        );
        assert_eq!(normalize_gemini_code_assist_endpoint(" \t ").unwrap(), None);
    }

    #[test]
    fn code_assist_policy_preserves_tier_label_semantics() {
        assert_eq!(
            gemini_code_assist_tier_label(Some("free-tier"), None).unwrap(),
            Some("free".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some(" Custom-tier "), Some("ignored")).unwrap(),
            Some("custom".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some(" FREE-TIER "), None).unwrap(),
            Some("free-tier".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some("-tier"), None).unwrap(),
            Some(String::new())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some("  "), Some("Google One AI ULTRA plan")).unwrap(),
            Some("ultra".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(None, Some(" Standard workspace ")).unwrap(),
            Some("standard".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(None, Some("Custom Plan")).unwrap(),
            Some("Custom Plan".to_string())
        );
        assert_eq!(gemini_code_assist_tier_label(None, None).unwrap(), None);
    }
}
