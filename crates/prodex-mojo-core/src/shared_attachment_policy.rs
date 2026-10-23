use crate::MojoError;

mod path_codec;
pub use path_codec::{decode_session_path, render_session_path};

const ABI_VERSION: i64 = 1;
const IMAGE_TAG_RANGE: i64 = 0;
const CLIPBOARD_RANGE: i64 = 1;
const ATTACHMENT_RANGE: i64 = 2;
const CLIPBOARD_NAME: i64 = 3;
const PERSISTABLE_NAME: i64 = 4;
const ROLLOUT_NAME: i64 = 5;
const GOAL_OBJECTIVE_NAME: i64 = 6;
const SQLITE_HEADER: i64 = 7;
const DYNAMIC_FILE_NAME: i64 = 8;
const MAX_BYTES: usize = 67_108_864;

/// Preserve the different scan cursors used by observation and path rewriting.
#[derive(Debug, Clone, Copy)]
pub enum ImageTagScan {
    WholeTags,
    RewritePaths,
}

/// Checked UTF-8 byte range selected from the original session text by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ImageTagPathRange {
    pub start: usize,
    pub end: usize,
}

const _: () = assert!(std::mem::size_of::<ImageTagPathRange>() == 16);

unsafe extern "C" {
    fn prodex_shared_image_tag_paths_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        required_address: u64,
    ) -> i64;
    fn prodex_shared_attachment_policy_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        cursor: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn call_bytes(operation: i64, input: &[u8], cursor: usize) -> Result<[i64; 3], MojoError> {
    let mut output = [0_i64; 3];
    status(unsafe {
        prodex_shared_attachment_policy_v1(
            ABI_VERSION,
            operation,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(cursor).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

fn call(operation: i64, input: &str, cursor: usize) -> Result<[i64; 3], MojoError> {
    call_bytes(operation, input.as_bytes(), cursor)
}

fn optional_range(
    output: [i64; 3],
    input: &str,
    minimum_start: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    match output[0] {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
            if start < minimum_start
                || start > end
                || end > input.len()
                || !input.is_char_boundary(start)
                || !input.is_char_boundary(end)
            {
                return Err(MojoError::InvalidOutput);
            }
            Ok(Some((start, end)))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_output(output: [i64; 3]) -> Result<bool, MojoError> {
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn image_tag_path_range(tag: &str) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(IMAGE_TAG_RANGE, tag, 0)?, tag, 0)
}

/// Batch the existing image-tag lexer and scan progression in the compiled owner.
pub fn image_tag_path_ranges(
    contents: &str,
    scan: ImageTagScan,
) -> Result<Vec<ImageTagPathRange>, MojoError> {
    if contents.len() > MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let call = |rows: &mut [ImageTagPathRange], required: &mut i64| unsafe {
        prodex_shared_image_tag_paths_v1(
            ABI_VERSION,
            match scan {
                ImageTagScan::WholeTags => 0,
                ImageTagScan::RewritePaths => 1,
            },
            contents.as_ptr() as usize as u64,
            contents.len() as i64,
            rows.as_mut_ptr() as usize as u64,
            rows.len() as i64,
            (required as *mut i64) as usize as u64,
        )
    };
    let mut required = -1_i64;
    let probe_status = call(&mut [], &mut required);
    if probe_status != 3 {
        status(probe_status)?;
    }
    let count = usize::try_from(required).map_err(|_| MojoError::InvalidOutput)?;
    if count > contents.len() / 7 {
        return Err(MojoError::InvalidOutput);
    }
    let mut rows = vec![ImageTagPathRange { start: 0, end: 0 }; count];
    status(call(&mut rows, &mut required))?;
    if usize::try_from(required).ok() != Some(count) {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous_end = 0;
    for row in &rows {
        if row.start < previous_end
            || row.start > row.end
            || contents.get(row.start..row.end).is_none()
        {
            return Err(MojoError::InvalidOutput);
        }
        previous_end = row.end;
    }
    Ok(rows)
}

pub fn next_clipboard_path(
    contents: &str,
    cursor: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(CLIPBOARD_RANGE, contents, cursor)?, contents, cursor)
}

pub fn next_attachment_path(
    contents: &str,
    cursor: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(ATTACHMENT_RANGE, contents, cursor)?, contents, cursor)
}

pub fn clipboard_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(CLIPBOARD_NAME, file_name, 0)?)
}

pub fn persistable_attachment_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(PERSISTABLE_NAME, file_name, 0)?)
}

/// Whether an attachment is a generated goal wrapper whose nested references
/// may be repaired. Raw user pasted text and image payloads remain opaque.
pub fn goal_objective_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(GOAL_OBJECTIVE_NAME, file_name, 0)?)
}

pub fn rollout_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(ROLLOUT_NAME, file_name, 0)?)
}

pub fn sqlite_header_is_sqlite(header: &[u8]) -> Result<bool, MojoError> {
    bool_output(call_bytes(SQLITE_HEADER, header, 0)?)
}

pub fn shared_codex_file_name_is_shared(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(DYNAMIC_FILE_NAME, file_name, 0)?)
}

#[cfg(test)]
#[path = "shared_attachment_policy/image_tag_batch_tests.rs"]
mod image_tag_batch_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_attachment_policy_preserves_scanner_contracts() {
        assert_eq!(
            image_tag_path_range("<image path=\\\"/tmp/codex-clipboard-a.png\\\"").unwrap(),
            Some((14, 40))
        );
        assert_eq!(
            image_tag_path_range(r#"<image path="/tmp/codex-clipboard-a.png""#).unwrap(),
            Some((13, 39))
        );

        let clipboard = "see /tmp/codex-clipboard-a.png.";
        let range = next_clipboard_path(clipboard, 0).unwrap().unwrap();
        assert_eq!(&clipboard[range.0..range.1], "/tmp/codex-clipboard-a.png");

        let attachment = r#"see /tmp/attachments/thread/image-a.png, next"#;
        let range = next_attachment_path(attachment, 0).unwrap().unwrap();
        assert_eq!(
            &attachment[range.0..range.1],
            "/tmp/attachments/thread/image-a.png"
        );

        let adjacent_escaped =
            r#"/tmp/attachments/first/image-1.png\n/tmp/attachments/second/image-2.png"#;
        let first = next_attachment_path(adjacent_escaped, 0).unwrap().unwrap();
        assert_eq!(
            &adjacent_escaped[first.0..first.1],
            "/tmp/attachments/first/image-1.png"
        );
        let second = next_attachment_path(adjacent_escaped, first.1)
            .unwrap()
            .unwrap();
        assert!(second.0 >= first.1);
        assert_eq!(
            &adjacent_escaped[second.0..second.1],
            "/tmp/attachments/second/image-2.png"
        );

        assert!(clipboard_file_name("codex-clipboard-a.png").unwrap());
        assert!(persistable_attachment_file_name("pasted-text-1.txt").unwrap());
        assert!(persistable_attachment_file_name("image-1.png").unwrap());
        assert!(persistable_attachment_file_name("goal-objective.md").unwrap());
        assert!(goal_objective_file_name("goal-objective.md").unwrap());
        for name in [
            "pasted-text-1.txt",
            "image-1.png",
            "other.md",
            "goal-objective.md.bak",
            "../goal-objective.md",
        ] {
            assert!(
                !goal_objective_file_name(name).unwrap(),
                "never rewrite raw user attachment {name}"
            );
        }
        assert!(!persistable_attachment_file_name("other.txt").unwrap());
        assert!(rollout_file_name("rollout.jsonl").unwrap());
        assert!(rollout_file_name("rollout.jsonl.zst").unwrap());
        assert!(!rollout_file_name("rollout.json").unwrap());
    }

    #[test]
    fn sqlite_header_policy_compares_raw_bytes_without_utf8_fallback() {
        assert!(sqlite_header_is_sqlite(b"SQLite format 3\0").unwrap());
        assert!(!sqlite_header_is_sqlite(&[0xff; 16]).unwrap());
        assert!(!sqlite_header_is_sqlite(b"SQLite format 3").unwrap());
    }

    #[test]
    fn shared_codex_file_name_policy_covers_config_and_sqlite_sidecars() {
        for name in [
            "local_1-prod.config.toml",
            "state_1.sqlite",
            "logs_1.sqlite-shm",
            "goals_1.sqlite-wal",
            "memories_1.sqlite",
        ] {
            assert!(
                shared_codex_file_name_is_shared(name).unwrap(),
                "expected shared Codex filename: {name}"
            );
        }
        for name in [
            ".config.toml",
            "team.prod.config.toml",
            "../prod.config.toml",
            "queue_1.sqlite",
            "state_1.sqlite-journal",
        ] {
            assert!(
                !shared_codex_file_name_is_shared(name).unwrap(),
                "unexpected shared Codex filename: {name}"
            );
        }
    }

    #[test]
    fn shared_codex_file_name_policy_rejects_invalid_utf8_at_the_abi() {
        assert_eq!(
            call_bytes(DYNAMIC_FILE_NAME, &[0xff], 0),
            Err(MojoError::InvalidInput)
        );
    }
}
