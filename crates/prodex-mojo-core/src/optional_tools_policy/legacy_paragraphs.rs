//! Compiled exact legacy Caveman paragraph digest removal plan.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_PARAGRAPHS: usize = 65_536;

unsafe extern "C" {
    fn prodex_optional_tool_legacy_instruction_paragraphs_v1(
        abi_version: i64,
        digests_address: u64,
        count: i64,
        indices_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Return strictly increasing indices of paragraphs NOT matching the retired
/// Caveman SHA-256 digest. Rust owns SHA-256 computation and TOML mutation.
pub fn optional_tool_retained_legacy_paragraph_indices(
    digests: &[[u8; 32]],
) -> Result<Vec<usize>, MojoError> {
    if digests.len() > MAX_PARAGRAPHS {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![-1_i64; digests.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_optional_tool_legacy_instruction_paragraphs_v1(
            ABI_VERSION,
            digests.as_ptr() as usize as u64,
            i64::try_from(digests.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > digests.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = None;
    output
        .into_iter()
        .take(count)
        .map(|raw| {
            let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
            if index >= digests.len() || previous.is_some_and(|prev| prev >= index) {
                return Err(MojoError::InvalidOutput);
            }
            previous = Some(index);
            Ok(index)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RETIRED_DIGEST: [u8; 32] = [
        0xa0, 0x7e, 0x8d, 0x51, 0x67, 0xe4, 0x54, 0xf7, 0x63, 0x7e, 0xb0, 0xe3, 0x52, 0x30, 0xa8,
        0x49, 0x87, 0xc5, 0x37, 0xae, 0x98, 0xd6, 0xf7, 0x95, 0xd2, 0x46, 0x33, 0x8b, 0x65, 0x28,
        0x10, 0xf1,
    ];

    #[test]
    fn compiled_legacy_paragraph_selection_removes_only_exact_digest() {
        let other = [0_u8; 32];
        let near = {
            let mut near = RETIRED_DIGEST;
            near[31] ^= 1;
            near
        };
        assert_eq!(
            optional_tool_retained_legacy_paragraph_indices(&[
                other,
                RETIRED_DIGEST,
                near,
                RETIRED_DIGEST,
                other
            ]),
            Ok(vec![0, 2, 4])
        );
        assert_eq!(
            optional_tool_retained_legacy_paragraph_indices(&[]),
            Ok(vec![])
        );
        assert_eq!(
            optional_tool_retained_legacy_paragraph_indices(&[RETIRED_DIGEST]),
            Ok(vec![])
        );
    }
}
