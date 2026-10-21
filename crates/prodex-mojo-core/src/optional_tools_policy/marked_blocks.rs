//! UTF-8 marked AGENTS.md instruction block deletion owned by Mojo.
const ABI_VERSION: i64 = 1;
const MAX_TEXT_BYTES: usize = 1_048_576;

unsafe extern "C" {
    fn prodex_optional_tool_marked_block_strip_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        begin_address: u64,
        begin_length: i64,
        end_address: u64,
        end_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkedBlockStripError {
    InvalidInput,
    MissingEnd,
    MissingBegin,
    AbiMismatch,
    InvalidOutput,
}

/// Delete all inclusive marked blocks, then reject unmatched closing markers.
/// Rust only borrows strings and hosts output. It never scans for markers.
pub fn strip_marked_instruction_blocks(
    text: &str,
    begin: &str,
    end: &str,
) -> Result<String, MarkedBlockStripError> {
    if text.len() > MAX_TEXT_BYTES
        || begin.is_empty()
        || end.is_empty()
        || begin.len() > MAX_TEXT_BYTES
        || end.len() > MAX_TEXT_BYTES
    {
        return Err(MarkedBlockStripError::InvalidInput);
    }
    let mut output = vec![0_u8; text.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_optional_tool_marked_block_strip_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MarkedBlockStripError::InvalidInput)?,
            begin.as_ptr() as usize as u64,
            i64::try_from(begin.len()).map_err(|_| MarkedBlockStripError::InvalidInput)?,
            end.as_ptr() as usize as u64,
            i64::try_from(end.len()).map_err(|_| MarkedBlockStripError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MarkedBlockStripError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MarkedBlockStripError::InvalidInput),
        2 => return Err(MarkedBlockStripError::MissingEnd),
        3 => return Err(MarkedBlockStripError::MissingBegin),
        4 => return Err(MarkedBlockStripError::AbiMismatch),
        _ => return Err(MarkedBlockStripError::InvalidOutput),
    }
    let length = usize::try_from(written).map_err(|_| MarkedBlockStripError::InvalidOutput)?;
    if length > text.len() {
        return Err(MarkedBlockStripError::InvalidOutput);
    }
    output.truncate(length);
    String::from_utf8(output).map_err(|_| MarkedBlockStripError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_marked_blocks_remove_multiple_pairs_and_preserve_unicode() {
        assert_eq!(
            strip_marked_instruction_blocks(
                "prefix🧭[BEGIN]first[END]中[BEGIN]second[END]tail",
                "[BEGIN]",
                "[END]",
            ),
            Ok("prefix🧭中tail".to_string())
        );
        assert_eq!(
            strip_marked_instruction_blocks("KEEP", "[BEGIN]", "[END]"),
            Ok("KEEP".to_string())
        );
        assert_eq!(
            strip_marked_instruction_blocks("[BEGIN]body[END]", "[BEGIN]", "[END]"),
            Ok(String::new())
        );
        assert_eq!(
            strip_marked_instruction_blocks("中🧭", "中", "🧭"),
            Ok(String::new())
        );
    }

    #[test]
    fn compiled_marked_blocks_rescan_after_joining_removed_segments() {
        // Removing the first block creates a second marker across the seam.
        let text = "prefix[BE[BEGIN]inside[END]GIN]next[END]tail";
        assert_eq!(
            strip_marked_instruction_blocks(text, "[BEGIN]", "[END]"),
            Ok("prefixtail".to_string())
        );
        // Empty and ordinary strings remain unaffected.
        assert_eq!(
            strip_marked_instruction_blocks("", "[BEGIN]", "[END]"),
            Ok(String::new())
        );
    }

    #[test]
    fn compiled_marked_blocks_reject_unpaired_markers() {
        assert_eq!(
            strip_marked_instruction_blocks("a[BEGIN]b", "[BEGIN]", "[END]"),
            Err(MarkedBlockStripError::MissingEnd)
        );
        assert_eq!(
            strip_marked_instruction_blocks("a[END]b", "[BEGIN]", "[END]"),
            Err(MarkedBlockStripError::MissingBegin)
        );
        assert_eq!(
            strip_marked_instruction_blocks("[END][BEGIN]text[END]", "[BEGIN]", "[END]"),
            Err(MarkedBlockStripError::MissingBegin)
        );
        assert_eq!(
            strip_marked_instruction_blocks("", "", "[END]"),
            Err(MarkedBlockStripError::InvalidInput)
        );
        assert_eq!(
            strip_marked_instruction_blocks(&"x".repeat(MAX_TEXT_BYTES + 1), "[BEGIN]", "[END]"),
            Err(MarkedBlockStripError::InvalidInput)
        );
    }
}
