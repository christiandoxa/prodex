//! A canonical managed AGENTS reference plan over borrowed UTF-8.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_optional_tool_agents_reference_update_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        reference_address: u64,
        reference_length: i64,
        output_address: u64,
        output_capacity: i64,
        result_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsReferenceUpdate {
    pub content: String,
    pub write_needed: bool,
}

/// Normalize and deduplicate managed AGENTS file references in compiled Mojo.
/// Rust does not search lines or redo the Unicode whitespace policy.
pub fn agents_reference_update(
    original: &str,
    reference: &str,
) -> Result<AgentsReferenceUpdate, MojoError> {
    if original.len() > MAX_INPUT_BYTES || reference.is_empty() || reference.len() > MAX_INPUT_BYTES
    {
        return Err(MojoError::InvalidInput);
    }
    let capacity = original
        .len()
        .checked_add(reference.len())
        .and_then(|n| n.checked_add(3))
        .ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; capacity];
    let mut result = [-1_i64; 2];
    let status = unsafe {
        prodex_optional_tool_agents_reference_update_v1(
            ABI_VERSION,
            original.as_ptr() as usize as u64,
            i64::try_from(original.len()).map_err(|_| MojoError::InvalidInput)?,
            reference.as_ptr() as usize as u64,
            i64::try_from(reference.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::Capacity)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let length = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(length);
    let write_needed = match result[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let content = String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)?;
    Ok(AgentsReferenceUpdate {
        content,
        write_needed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_reference_update_deduplicates_and_is_idempotent() {
        let reference = "@/home/test-user/SUB_AGENTS.md";
        let original = format!("intro\n  {reference}  \nother\n{reference}\n");
        let expected = format!("intro\nother\n\n{reference}\n");
        let plan = agents_reference_update(&original, reference).unwrap();
        assert_eq!(plan.content, expected);
        assert!(plan.write_needed);
        let repeated = agents_reference_update(&plan.content, reference).unwrap();
        assert_eq!(repeated.content, plan.content);
        assert!(!repeated.write_needed);
    }

    #[test]
    fn compiled_reference_update_preserves_unicode_whitespace_and_crlf_rules() {
        let reference = "@日本語.md";
        assert_eq!(
            agents_reference_update("  \u{2003}\n", reference)
                .unwrap()
                .content,
            format!("{reference}\n")
        );
        assert_eq!(
            agents_reference_update("keep\r\n@日本語.md\r\n", reference)
                .unwrap()
                .content,
            format!("keep\n\n{reference}\n")
        );
        assert_eq!(
            agents_reference_update("other\n", reference)
                .unwrap()
                .content,
            format!("other\n\n{reference}\n")
        );
        assert_eq!(
            agents_reference_update("", reference).unwrap().content,
            format!("{reference}\n")
        );
        assert_eq!(
            agents_reference_update("a", ""),
            Err(MojoError::InvalidInput)
        );
    }
}
