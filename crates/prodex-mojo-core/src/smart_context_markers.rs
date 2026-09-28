use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_FIELD_BYTES: usize = 512;
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
}
