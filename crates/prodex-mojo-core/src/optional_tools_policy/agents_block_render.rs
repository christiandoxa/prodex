//! Versioned rendered managed AGENTS instruction block transitions.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_TEXT_BYTES: usize = 1_048_576;
const VIEW_COUNT: usize = 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct TextView {
    address: u64,
    length: u64,
}

unsafe extern "C" {
    fn prodex_optional_tool_agents_block_render_v1(
        abi_version: i64,
        operation: i64,
        views_address: u64,
        view_count: i64,
        output_address: u64,
        output_capacity: i64,
        result_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsBlockOperation {
    Upsert,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsBlockRender {
    pub content: String,
    pub write_needed: bool,
}

/// Compile the final insertion/deletion body over the already Mojo-stripped
/// content. No host Unicode trim, newline formatting, or duplicate decision.
pub fn agents_block_render(
    operation: AgentsBlockOperation,
    original: &str,
    cleaned: &str,
    begin: &str,
    end: &str,
    block: &str,
) -> Result<AgentsBlockRender, MojoError> {
    let fields = [original, cleaned, begin, end, block];
    if fields.iter().any(|value| value.len() > MAX_TEXT_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = fields.map(|value| TextView {
        address: value.as_ptr() as usize as u64,
        length: value.len() as u64,
    });
    let capacity = fields[1..]
        .iter()
        .try_fold(8usize, |sum, value| sum.checked_add(value.len()))
        .ok_or(MojoError::Capacity)?;
    let mut bytes = vec![0u8; capacity];
    let mut result = [-1_i64; 2];
    let status = unsafe {
        prodex_optional_tool_agents_block_render_v1(
            ABI_VERSION,
            match operation {
                AgentsBlockOperation::Upsert => 0,
                AgentsBlockOperation::Remove => 1,
            },
            views.as_ptr() as usize as u64,
            VIEW_COUNT as i64,
            bytes.as_mut_ptr() as usize as u64,
            i64::try_from(bytes.len()).map_err(|_| MojoError::Capacity)?,
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
    let len = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    if len > bytes.len() {
        return Err(MojoError::InvalidOutput);
    }
    bytes.truncate(len);
    let write_needed = match result[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if !write_needed && !bytes.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    let content = String::from_utf8(bytes).map_err(|_| MojoError::InvalidOutput)?;
    Ok(AgentsBlockRender {
        content,
        write_needed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_agents_block_insert_preserves_trimmed_join_contract() {
        let plan = agents_block_render(
            AgentsBlockOperation::Upsert,
            "alpha",
            "alpha  \n",
            "[BEGIN]",
            "[END]",
            "  🌍 hello\n ",
        )
        .unwrap();
        assert!(plan.write_needed);
        assert_eq!(plan.content, "alpha\n\n[BEGIN]\n🌍 hello\n[END]\n");
        let empty = agents_block_render(
            AgentsBlockOperation::Upsert,
            " \u{2003} ",
            " \u{2003} ",
            "[BEGIN]",
            "[END]",
            " ",
        )
        .unwrap();
        assert_eq!(empty.content, "[BEGIN]\n\n[END]\n");
    }

    #[test]
    fn compiled_agents_block_remove_noops_if_unmodified() {
        let no_change = agents_block_render(
            AgentsBlockOperation::Remove,
            "no markers\n",
            "no markers\n",
            "",
            "",
            "",
        )
        .unwrap();
        assert!(!no_change.write_needed);
        assert!(no_change.content.is_empty());
        let changed = agents_block_render(
            AgentsBlockOperation::Remove,
            "keep\n[BEGIN]removed[END]\n",
            "keep\n\n",
            "",
            "",
            "",
        )
        .unwrap();
        assert!(changed.write_needed);
        assert_eq!(changed.content, "keep");
    }
}
