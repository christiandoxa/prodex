use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
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
