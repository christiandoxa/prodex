//! Shape-only Gemini fast-path plans; text payloads and callback arguments stay in Rust.

use super::{MojoError, RichStringView, ensure_rich_abi, view};

/// Borrowed JSON facts, with `None` keys denoting a non-object part.
pub struct GeminiSimplePartInput<'a> {
    pub keys: Option<&'a [&'a str]>,
    pub text_present: bool,
    pub text_is_string: bool,
    pub thought: bool,
    pub function_call_present: bool,
    pub function_call_is_object: bool,
    pub function_name: Option<&'a str>,
}

/// Ordered host actions, including rejection that must follow a callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiSimplePartAction {
    Reject,
    Skip,
    CheckCall,
    CheckCallThenReject,
}

/// The message-only shortcut or an ordered callback plan for the original parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeminiSimpleResponsePlan {
    MessageOnly,
    CheckParts(Vec<GeminiSimplePartAction>),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct Part {
    keys_address: u64,
    key_count: i64,
    name: RichStringView,
    flags: u64,
}
const _: () = assert!(std::mem::size_of::<Part>() == 40);

unsafe extern "C" {
    fn prodex_mojo_gemini_simple_response_plan_v1(
        version: i64,
        parts_address: u64,
        count: i64,
        output_address: u64,
        output_words: i64,
    ) -> i64;
}

pub fn plan_gemini_simple_response(
    inputs: &[GeminiSimplePartInput<'_>],
) -> Result<GeminiSimpleResponsePlan, MojoError> {
    ensure_rich_abi()?;
    let keys = inputs
        .iter()
        .map(|part| {
            part.keys
                .unwrap_or_default()
                .iter()
                .map(|key| view(key))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let parts = inputs
        .iter()
        .zip(&keys)
        .map(|(part, keys)| {
            Ok(Part {
                keys_address: keys.as_ptr() as u64,
                key_count: i64::try_from(keys.len()).map_err(|_| MojoError::InvalidInput)?,
                name: part.function_name.map(view).unwrap_or_default(),
                flags: u64::from(part.keys.is_some())
                    | (u64::from(part.text_present) << 1)
                    | (u64::from(part.text_is_string) << 2)
                    | (u64::from(part.thought) << 3)
                    | (u64::from(part.function_call_present) << 4)
                    | (u64::from(part.function_call_is_object) << 5),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let capacity = inputs.len().checked_add(1).ok_or(MojoError::Capacity)?;
    let mut output = vec![-1_i64; capacity];
    let status = unsafe {
        prodex_mojo_gemini_simple_response_plan_v1(
            1,
            parts.as_ptr() as u64,
            i64::try_from(parts.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 | 2 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output[0] {
        0 => Ok(GeminiSimpleResponsePlan::MessageOnly),
        1 => output[1..]
            .iter()
            .zip(inputs)
            .map(|(action, input)| match action {
                0 => Ok(GeminiSimplePartAction::Reject),
                1 => Ok(GeminiSimplePartAction::Skip),
                2 | 3 if input.function_call_is_object && input.function_name.is_some() => {
                    Ok(if *action == 2 {
                        GeminiSimplePartAction::CheckCall
                    } else {
                        GeminiSimplePartAction::CheckCallThenReject
                    })
                }
                _ => Err(MojoError::InvalidOutput),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(GeminiSimpleResponsePlan::CheckParts),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "gemini_simple_response_tests.rs"]
mod tests;
