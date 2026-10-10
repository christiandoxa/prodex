use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_FIELD_BYTES: usize = 512;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_RANGES: usize = 256;
const MAX_EXCERPT_BYTES: usize = 16 * 1024;
const RECORD_WIDTH: usize = 15;
const NONE: u64 = u64::MAX;

#[repr(i64)]
#[derive(Clone, Copy)]
enum Operation {
    PathLooksLikeFile = 0,
    FileLocationToken = 1,
    DiffFilePath = 2,
    DiffSpan = 3,
    TestFailure = 4,
    TestSymbol = 5,
    ErrorCode = 6,
    CommandLineKind = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileLocationPlan {
    pub path_start: usize,
    pub path_end: usize,
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSpanPlan {
    pub start: usize,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandLineKind {
    Python,
    Diff,
    CargoTest,
    CargoBuild,
    NpmTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRangeKind {
    FileLocation,
    DiffHunk,
    TestFailure,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticRangePlan {
    pub kind: SemanticRangeKind,
    pub start_line: usize,
    pub end_line: usize,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub old_start: Option<usize>,
    pub old_count: Option<usize>,
    pub new_start: Option<usize>,
    pub new_count: Option<usize>,
    pub code: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticIndexPlan {
    pub complete: bool,
    pub command_kind: Option<CommandLineKind>,
    pub ranges: Vec<SemanticRangePlan>,
}

unsafe extern "C" {
    fn prodex_smart_context_markers_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
        aux: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        meta_address: u64,
    ) -> i64;
    fn prodex_smart_context_semantic_index_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        line_spans_address: u64,
        line_count: i64,
        max_ranges: i64,
        max_excerpt_bytes: i64,
        output_address: u64,
        output_capacity: i64,
        field_address: u64,
        field_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

struct CallResult {
    output: Vec<u8>,
    written: usize,
    meta: [u64; 4],
}

fn call(value: &str, operation: Operation, aux: u64) -> Result<CallResult, MojoError> {
    let mut output = vec![0_u8; MAX_FIELD_BYTES];
    let mut written = 0_i64;
    let mut meta = [NONE; 4];
    let status = unsafe {
        prodex_smart_context_markers_v1(
            ABI_VERSION,
            operation as i64,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            aux,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            meta.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(CallResult {
        output,
        written,
        meta,
    })
}

fn boolean(value: u64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn index(value: u64) -> Result<usize, MojoError> {
    usize::try_from(value).map_err(|_| MojoError::InvalidOutput)
}

fn optional_index(value: u64) -> Result<Option<usize>, MojoError> {
    if value == NONE {
        Ok(None)
    } else {
        index(value).map(Some)
    }
}

fn span(meta: &[u64; 4], len: usize) -> Result<Option<(usize, usize)>, MojoError> {
    if meta[0] == NONE {
        return Ok(None);
    }
    let start = index(meta[0])?;
    let end = index(meta[1])?;
    if start > end || end > len {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}

pub fn path_looks_like_file(path: &str) -> Result<bool, MojoError> {
    boolean(call(path, Operation::PathLooksLikeFile, 0)?.meta[0])
}

pub fn parse_file_location_token(token: &str) -> Result<Option<FileLocationPlan>, MojoError> {
    let result = call(token, Operation::FileLocationToken, 0)?;
    let Some((path_start, path_end)) = span(&result.meta, token.len())? else {
        return Ok(None);
    };
    let line = index(result.meta[2])?;
    let column = optional_index(result.meta[3])?;
    Ok(Some(FileLocationPlan {
        path_start,
        path_end,
        line,
        column,
    }))
}

pub fn normalize_diff_file_path_token(token: &str) -> Result<Option<(usize, usize)>, MojoError> {
    let result = call(token, Operation::DiffFilePath, 0)?;
    span(&result.meta, token.len())
}

pub fn parse_diff_span(span: &str, prefix: char) -> Result<Option<DiffSpanPlan>, MojoError> {
    let prefix = match prefix {
        '-' => b'-',
        '+' => b'+',
        _ => return Err(MojoError::InvalidInput),
    };
    let result = call(span, Operation::DiffSpan, u64::from(prefix))?;
    if result.meta[0] == NONE {
        return Ok(None);
    }
    Ok(Some(DiffSpanPlan {
        start: index(result.meta[0])?,
        count: index(result.meta[1])?,
    }))
}

pub fn is_test_failure_line(line: &str) -> Result<bool, MojoError> {
    boolean(call(line, Operation::TestFailure, 0)?.meta[0])
}

pub fn test_symbol_span(line: &str) -> Result<Option<(usize, usize)>, MojoError> {
    let result = call(line, Operation::TestSymbol, 0)?;
    span(&result.meta, line.len())
}

pub fn error_code(line: &str) -> Result<Option<String>, MojoError> {
    let result = call(line, Operation::ErrorCode, 0)?;
    if result.written == 0 {
        return Ok(None);
    }
    String::from_utf8(result.output[..result.written].to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn command_line_kind(line: &str) -> Result<Option<CommandLineKind>, MojoError> {
    let result = call(line, Operation::CommandLineKind, 0)?;
    match result.meta[0] {
        0 => Ok(None),
        1 => Ok(Some(CommandLineKind::Python)),
        2 => Ok(Some(CommandLineKind::Diff)),
        3 => Ok(Some(CommandLineKind::CargoTest)),
        4 => Ok(Some(CommandLineKind::CargoBuild)),
        5 => Ok(Some(CommandLineKind::NpmTest)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn semantic_line_spans(text: &str, lines: &[&str]) -> Result<Vec<u64>, MojoError> {
    let text_address = text.as_ptr() as usize;
    let mut spans = Vec::with_capacity(lines.len().saturating_mul(2));
    for line in lines {
        let start = (line.as_ptr() as usize)
            .checked_sub(text_address)
            .ok_or(MojoError::InvalidInput)?;
        let end = start
            .checked_add(line.len())
            .ok_or(MojoError::InvalidInput)?;
        if text.get(start..end) != Some(*line) {
            return Err(MojoError::InvalidInput);
        }
        spans.push(u64::try_from(start).map_err(|_| MojoError::InvalidInput)?);
        spans.push(u64::try_from(end).map_err(|_| MojoError::InvalidInput)?);
    }
    Ok(spans)
}

fn decode_semantic_field(
    fields: &[u8],
    written: usize,
    start: u64,
    length: u64,
) -> Result<Option<String>, MojoError> {
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    if length == 0 {
        if start != NONE {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            if start > written {
                return Err(MojoError::InvalidOutput);
            }
        }
        return Ok(None);
    }
    if start == NONE || length > MAX_FIELD_BYTES {
        return Err(MojoError::InvalidOutput);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    if end > written || end > fields.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(fields[start..end].to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

fn decode_semantic_optional(value: u64) -> Result<Option<usize>, MojoError> {
    (value != NONE)
        .then(|| usize::try_from(value).map_err(|_| MojoError::InvalidOutput))
        .transpose()
}

fn decode_semantic_kind(value: u64) -> Result<SemanticRangeKind, MojoError> {
    match value {
        1 => Ok(SemanticRangeKind::FileLocation),
        2 => Ok(SemanticRangeKind::DiffHunk),
        3 => Ok(SemanticRangeKind::TestFailure),
        4 => Ok(SemanticRangeKind::Error),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_semantic_command_kind(value: u64) -> Result<Option<CommandLineKind>, MojoError> {
    match value {
        0 => Ok(None),
        1 => Ok(Some(CommandLineKind::Python)),
        2 => Ok(Some(CommandLineKind::Diff)),
        3 => Ok(Some(CommandLineKind::CargoTest)),
        4 => Ok(Some(CommandLineKind::CargoBuild)),
        5 => Ok(Some(CommandLineKind::NpmTest)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans the bounded Smart Context semantic index in one versioned Mojo call.
/// Rust only supplies borrowed line spans and materializes validated strings and DTOs.
pub fn semantic_index(
    text: &str,
    lines: &[&str],
    max_ranges: usize,
    max_excerpt_bytes: usize,
) -> Result<SemanticIndexPlan, MojoError> {
    if text.len() > MAX_INPUT_BYTES
        || max_ranges > MAX_RANGES
        || max_excerpt_bytes > MAX_EXCERPT_BYTES
    {
        return Err(MojoError::InvalidInput);
    }
    let line_spans = semantic_line_spans(text, lines)?;
    let output_capacity = max_ranges
        .checked_mul(RECORD_WIDTH)
        .and_then(|capacity| capacity.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let field_capacity = max_ranges
        .checked_mul(MAX_FIELD_BYTES * 3)
        .and_then(|capacity| capacity.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u64; output_capacity];
    let mut fields = vec![0_u8; field_capacity];
    let mut metadata = [NONE; 4];
    let status = unsafe {
        prodex_smart_context_semantic_index_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            line_spans.as_ptr() as usize as u64,
            i64::try_from(lines.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_ranges).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_excerpt_bytes).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            fields.as_mut_ptr() as usize as u64,
            i64::try_from(fields.len()).map_err(|_| MojoError::InvalidInput)?,
            metadata.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(metadata[0]).map_err(|_| MojoError::InvalidOutput)?;
    let complete = match metadata[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let field_written = usize::try_from(metadata[3]).map_err(|_| MojoError::InvalidOutput)?;
    if count > max_ranges
        || count
            .checked_mul(RECORD_WIDTH)
            .is_none_or(|end| end > output.len())
        || field_written > fields.len()
    {
        return Err(MojoError::InvalidOutput);
    }

    let mut ranges = Vec::with_capacity(count);
    for record in output[..count * RECORD_WIDTH].as_chunks::<RECORD_WIDTH>().0 {
        let start_line = usize::try_from(record[1]).map_err(|_| MojoError::InvalidOutput)?;
        let end_line = usize::try_from(record[2]).map_err(|_| MojoError::InvalidOutput)?;
        if start_line == 0 || start_line > end_line || end_line > lines.len() {
            return Err(MojoError::InvalidOutput);
        }
        ranges.push(SemanticRangePlan {
            kind: decode_semantic_kind(record[0])?,
            start_line,
            end_line,
            path: decode_semantic_field(&fields, field_written, record[3], record[4])?,
            line: decode_semantic_optional(record[5])?,
            column: decode_semantic_optional(record[6])?,
            old_start: decode_semantic_optional(record[7])?,
            old_count: decode_semantic_optional(record[8])?,
            new_start: decode_semantic_optional(record[9])?,
            new_count: decode_semantic_optional(record[10])?,
            code: decode_semantic_field(&fields, field_written, record[11], record[12])?,
            symbol: decode_semantic_field(&fields, field_written, record[13], record[14])?,
        });
    }
    Ok(SemanticIndexPlan {
        complete,
        command_kind: decode_semantic_command_kind(metadata[2])?,
        ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_context_marker_kernel_smoke() {
        assert!(path_looks_like_file("src/main.rs").unwrap());
        assert!(!path_looks_like_file("not-a-file").unwrap());

        let location = parse_file_location_token("(file://a/src/main.rs:22:5)")
            .unwrap()
            .unwrap();
        assert_eq!(
            &"(file://a/src/main.rs:22:5)"[location.path_start..location.path_end],
            "src/main.rs"
        );
        assert_eq!(location.line, 22);
        assert_eq!(location.column, Some(5));

        let path = normalize_diff_file_path_token("a/src/main.rs")
            .unwrap()
            .unwrap();
        assert_eq!(&"a/src/main.rs"[path.0..path.1], "src/main.rs");

        assert_eq!(
            parse_diff_span("-20,2", '-').unwrap(),
            Some(DiffSpanPlan {
                start: 20,
                count: 2,
            })
        );
        assert!(is_test_failure_line("test result: FAILED. 0 passed").unwrap());
        assert_eq!(
            test_symbol_span("---- tests::fails stdout ----").unwrap(),
            Some((5, 17))
        );
        assert_eq!(
            error_code("error[E0277]: trait bound failed")
                .unwrap()
                .as_deref(),
            Some("E0277")
        );
        assert_eq!(
            error_code("process exit code \u{2003}17")
                .unwrap()
                .as_deref(),
            Some("exit_code_17")
        );
        assert_eq!(
            error_code("server status code \u{00a0}503")
                .unwrap()
                .as_deref(),
            Some("status_code_503")
        );
        let quoted_path = "\"a/src/lib.rs\"";
        let quoted_span = normalize_diff_file_path_token(quoted_path)
            .unwrap()
            .unwrap();
        assert_eq!(&quoted_path[quoted_span.0..quoted_span.1], "a/src/lib.rs");
        assert_eq!(
            command_line_kind("Traceback (most recent call last):").unwrap(),
            Some(CommandLineKind::Python)
        );
    }

    #[test]
    fn semantic_index_batch_plans_ranges_and_command_precedence() {
        let text = "running 1 test\n---- tests::fails stdout ----\nerror[E0277]: failed\n --> src/main.rs:22:5\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -20,2 +20,3 @@ fn demo()\n-old\n+new\ntest result: FAILED";
        let lines = text.lines().collect::<Vec<_>>();
        let plan = semantic_index(text, &lines, 32, 16 * 1024).unwrap();
        assert!(plan.complete);
        assert_eq!(plan.command_kind, Some(CommandLineKind::CargoTest));
        assert!(plan.ranges.iter().any(|range| {
            range.kind == SemanticRangeKind::FileLocation
                && range.path.as_deref() == Some("src/main.rs")
                && range.line == Some(22)
                && range.column == Some(5)
        }));
        assert!(plan.ranges.iter().any(|range| {
            range.kind == SemanticRangeKind::DiffHunk
                && range.path.as_deref() == Some("src/main.rs")
                && range.old_start == Some(20)
                && range.new_count == Some(3)
        }));
        assert!(plan.ranges.iter().any(|range| {
            range.kind == SemanticRangeKind::TestFailure
                && range.symbol.as_deref() == Some("tests::fails")
        }));
        assert!(plan.ranges.iter().any(|range| {
            range.kind == SemanticRangeKind::Error && range.code.as_deref() == Some("E0277")
        }));
    }

    #[test]
    fn semantic_index_batch_reports_bounded_capacity() {
        let text = (0..4)
            .map(|index| format!("src/file{index}.rs:1:1"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = text.lines().collect::<Vec<_>>();
        let plan = semantic_index(text.as_str(), &lines, 2, 16 * 1024).unwrap();
        assert_eq!(plan.ranges.len(), 2);
        assert!(!plan.complete);

        let duplicate_text = "test result: FAILED\ntest result: FAILED";
        let duplicate_lines = duplicate_text.lines().collect::<Vec<_>>();
        let duplicate = semantic_index(duplicate_text, &duplicate_lines, 8, 16 * 1024).unwrap();
        assert_eq!(duplicate.ranges.len(), 1);
        assert!(duplicate.complete);
    }
}
