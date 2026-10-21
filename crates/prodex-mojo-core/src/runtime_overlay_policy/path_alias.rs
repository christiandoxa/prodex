//! Versioned compiled lexical path component policy over host path-kind tags.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_COMPONENTS: usize = 65_536;

unsafe extern "C" {
    fn prodex_runtime_overlay_lexical_path_v1(
        abi_version: i64,
        kinds_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Return original component indices selected by compiled Mojo. The host
/// owns non-UTF-8 OsStr bytes and never duplicates the stack decisions.
/// Kinds are Prefix=0, RootDir=1, CurDir=2, Normal=3, ParentDir=4.
pub fn lexical_workspace_component_indices(kinds: &[i64]) -> Result<Vec<usize>, MojoError> {
    if kinds.len() > MAX_COMPONENTS {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![-1_i64; kinds.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_overlay_lexical_path_v1(
            ABI_VERSION,
            kinds.as_ptr() as usize as u64,
            i64::try_from(kinds.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > kinds.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = None;
    output
        .into_iter()
        .take(count)
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            if index >= kinds.len() || previous.is_some_and(|prev| prev >= index) {
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

    #[test]
    fn compiled_component_stack_preserves_relative_and_rooted_semantics() {
        assert_eq!(lexical_workspace_component_indices(&[]), Ok(vec![]));
        assert_eq!(lexical_workspace_component_indices(&[3, 4]), Ok(vec![]));
        assert_eq!(
            lexical_workspace_component_indices(&[4, 4, 3, 4]),
            Ok(vec![0, 1])
        );
        assert_eq!(
            lexical_workspace_component_indices(&[1, 3, 3, 4, 4, 4]),
            Ok(vec![0])
        );
        assert_eq!(
            lexical_workspace_component_indices(&[0, 1, 3, 4, 4]),
            Ok(vec![0, 1])
        );
        assert_eq!(
            lexical_workspace_component_indices(&[3, 2, 3]),
            Ok(vec![0, 2])
        );
        assert_eq!(
            lexical_workspace_component_indices(&[9]),
            Err(MojoError::InvalidInput)
        );
    }
}
