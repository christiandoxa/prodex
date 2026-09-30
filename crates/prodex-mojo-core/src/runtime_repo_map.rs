use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 512;

unsafe extern "C" {
    fn prodex_runtime_repo_module_like_v1(abi_version: i64, address: u64, length: i64) -> i64;
    fn prodex_runtime_repo_chunk_plan_v1(
        abi_version: i64,
        current_count: i64,
        max_count: i64,
        byte_len: i64,
        text_len: i64,
        hash_matches: i64,
    ) -> i64;
    fn prodex_runtime_repo_duplicate_plan_v1(
        abi_version: i64,
        occurrence_count: i64,
        duplicate_count: i64,
        max_duplicate_count: i64,
        max_occurrences: i64,
    ) -> i64;
    fn prodex_runtime_repo_path_distance_v1(
        abi_version: i64,
        start: i64,
        end: i64,
        line: i64,
    ) -> i64;
    fn prodex_runtime_repo_symbol_kind_v1(
        abi_version: i64,
        label_address: u64,
        label_length: i64,
        text_address: u64,
        text_length: i64,
    ) -> i64;
    fn prodex_runtime_repo_entry_replace_v1(
        abi_version: i64,
        incoming_order: u64,
        current_order: u64,
        incoming_id_address: u64,
        incoming_id_length: i64,
        current_id_address: u64,
        current_id_length: i64,
    ) -> i64;
    fn prodex_runtime_repo_module_path_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn input_len(value: &str) -> Result<i64, MojoError> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoChunkPlan {
    Accept,
    RejectCapacity,
    RejectInvalid,
}

pub fn repo_chunk_plan(
    current_count: usize,
    max_count: usize,
    byte_len: usize,
    text_len: usize,
    hash_matches: bool,
) -> Result<RepoChunkPlan, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_chunk_plan_v1(
            ABI_VERSION,
            i64::try_from(current_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(byte_len).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(text_len).map_err(|_| MojoError::InvalidInput)?,
            i64::from(hash_matches),
        )
    };
    match result {
        0 => Ok(RepoChunkPlan::Accept),
        1 => Ok(RepoChunkPlan::RejectCapacity),
        2 => Ok(RepoChunkPlan::RejectInvalid),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoDuplicatePlan {
    Skip,
    Append { occurrences_complete: bool },
    StopCapacity,
}

pub fn repo_duplicate_plan(
    occurrence_count: usize,
    duplicate_count: usize,
    max_duplicate_count: usize,
    max_occurrences: usize,
) -> Result<RepoDuplicatePlan, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_duplicate_plan_v1(
            ABI_VERSION,
            i64::try_from(occurrence_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(duplicate_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_duplicate_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_occurrences).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if result == -1 {
        return Err(MojoError::InvalidInput);
    }
    match result & 0xff {
        0 => Ok(RepoDuplicatePlan::Skip),
        1 => Ok(RepoDuplicatePlan::Append {
            occurrences_complete: ((result >> 8) & 0xff) == 1,
        }),
        2 => Ok(RepoDuplicatePlan::StopCapacity),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_path_distance(start: usize, end: usize, line: usize) -> Result<usize, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_path_distance_v1(
            ABI_VERSION,
            i64::try_from(start).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(end).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(line).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if result < 0 {
        return Err(MojoError::InvalidInput);
    }
    usize::try_from(result).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSymbolKind {
    Module,
    Symbol,
    Test,
}

pub fn repo_symbol_kind(label: Option<&str>, text: &str) -> Result<RepoSymbolKind, MojoError> {
    let label = label.unwrap_or_default();
    let result = unsafe {
        prodex_runtime_repo_symbol_kind_v1(
            ABI_VERSION,
            label.as_ptr() as usize as u64,
            input_len(label)?,
            text.as_ptr() as usize as u64,
            input_len(text)?,
        )
    };
    match result {
        1 => Ok(RepoSymbolKind::Module),
        2 => Ok(RepoSymbolKind::Symbol),
        3 => Ok(RepoSymbolKind::Test),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_entry_should_replace(
    incoming_order: u64,
    current_order: u64,
    incoming_id: &str,
    current_id: &str,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_entry_replace_v1(
            ABI_VERSION,
            incoming_order,
            current_order,
            incoming_id.as_ptr() as usize as u64,
            input_len(incoming_id)?,
            current_id.as_ptr() as usize as u64,
            input_len(current_id)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_module_like(text: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_module_like_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            input_len(text)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_module_from_path(path: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; MAX_OUTPUT_BYTES];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_repo_module_path_v1(
            ABI_VERSION,
            path.as_ptr() as usize as u64,
            input_len(path)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status == 3 {
        return Ok(None);
    }
    if status != 0 {
        return Err(if status == 1 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    String::from_utf8(bytes.to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_map_kernel_matches_module_helpers() {
        for (text, expected) in [
            ("#[cfg(test)]\npub mod alpha {", true),
            ("// comment\n export default class Widget {", true),
            ("\u{3000}# comment\n\u{2003}pub(super) mod beta;", true),
            ("@decorator\nasync fn run() {}", false),
            ("pub async fn run() {}", false),
            ("pub fn run() {}", false),
            ("\n\r\nclass Widget:", true),
        ] {
            assert_eq!(repo_module_like(text).unwrap(), expected, "{text:?}");
        }

        assert_eq!(
            repo_chunk_plan(0, 4, 10, 10, true).unwrap(),
            RepoChunkPlan::Accept
        );
        assert_eq!(
            repo_chunk_plan(4, 4, 10, 10, true).unwrap(),
            RepoChunkPlan::RejectCapacity
        );
        assert_eq!(
            repo_chunk_plan(0, 4, 10, 9, true).unwrap(),
            RepoChunkPlan::RejectInvalid
        );
        assert_eq!(
            repo_duplicate_plan(3, 0, 8, 4).unwrap(),
            RepoDuplicatePlan::Append {
                occurrences_complete: true
            }
        );
        assert_eq!(repo_path_distance(10, 20, 15).unwrap(), 0);
        assert_eq!(repo_path_distance(10, 20, 25).unwrap(), 5);
        assert_eq!(
            repo_symbol_kind(Some("test_symbol"), "fn t() {}").unwrap(),
            RepoSymbolKind::Test
        );
        assert_eq!(
            repo_symbol_kind(Some("symbol"), "pub mod alpha {").unwrap(),
            RepoSymbolKind::Module
        );
        assert!(repo_entry_should_replace(2, 1, "b", "a").unwrap());
        assert!(repo_entry_should_replace(1, 1, "a", "b").unwrap());

        for (path, expected) in [
            ("src/foo/mod.rs", Some("foo".to_string())),
            ("src/lib.rs", Some("lib".to_string())),
            ("a/src/api/client.ts", Some("api::client".to_string())),
            ("b/app/index.js", Some("app".to_string())),
            ("./pkg/main.py", Some("pkg".to_string())),
            ("src\\api\\mod.rs", Some("api".to_string())),
            ("\"a/src/foo.rs\"", Some("a::src::foo".to_string())),
            ("dir.name/file", Some("dir".to_string())),
            (".", None),
            ("", None),
        ] {
            assert_eq!(repo_module_from_path(path).unwrap(), expected, "{path:?}");
        }
    }
}
