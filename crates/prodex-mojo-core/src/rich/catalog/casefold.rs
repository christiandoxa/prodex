use super::{
    MojoError, RICH_ABI_VERSION, count_address, ensure_rich_abi,
    prodex_mojo_rich_ascii_casefold_equal_v1, prodex_mojo_rich_ascii_casefold_find_v1,
    prodex_mojo_rich_ascii_casefold_relation_v1, status,
};

pub fn ascii_casefold_equal_exact(left: &str, right: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = -1_i64;
    status(unsafe {
        prodex_mojo_rich_ascii_casefold_equal_v1(
            RICH_ABI_VERSION,
            left.as_ptr() as usize as u64,
            i64::try_from(left.len()).map_err(|_| MojoError::InvalidInput)?,
            right.as_ptr() as usize as u64,
            i64::try_from(right.len()).map_err(|_| MojoError::InvalidInput)?,
            count_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsciiCasefoldRelation {
    StartsWith = 0,
    EndsWith = 1,
    Contains = 2,
}

fn ascii_casefold_relation(
    operation: AsciiCasefoldRelation,
    value: &str,
    needle: &str,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = -1_i64;
    status(unsafe {
        prodex_mojo_rich_ascii_casefold_relation_v1(
            RICH_ABI_VERSION,
            operation as i64,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            needle.as_ptr() as usize as u64,
            i64::try_from(needle.len()).map_err(|_| MojoError::InvalidInput)?,
            count_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn ascii_casefold_starts_with(value: &str, prefix: &str) -> Result<bool, MojoError> {
    ascii_casefold_relation(AsciiCasefoldRelation::StartsWith, value, prefix)
}

pub fn ascii_casefold_ends_with(value: &str, suffix: &str) -> Result<bool, MojoError> {
    ascii_casefold_relation(AsciiCasefoldRelation::EndsWith, value, suffix)
}

pub fn ascii_casefold_contains(value: &str, needle: &str) -> Result<bool, MojoError> {
    ascii_casefold_relation(AsciiCasefoldRelation::Contains, value, needle)
}

pub fn ascii_casefold_find(value: &str, needle: &str) -> Result<Option<usize>, MojoError> {
    ensure_rich_abi()?;
    let mut output_offset = -1_i64;
    status(unsafe {
        prodex_mojo_rich_ascii_casefold_find_v1(
            RICH_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            needle.as_ptr() as usize as u64,
            i64::try_from(needle.len()).map_err(|_| MojoError::InvalidInput)?,
            count_address(&mut output_offset),
        )
    })?;
    match output_offset {
        -1 => Ok(None),
        offset if offset >= 0 => usize::try_from(offset)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}
