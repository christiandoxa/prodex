use crate::MojoError;
use std::collections::HashSet;

const ABI_VERSION: i64 = 1;
const PROCESS_CPU: i64 = 0;
const SYSTEM_CPU: i64 = 1;
const KIB_FIELD: i64 = 2;
const U64_FIELD: i64 = 3;
const NETWORK_QUEUES: i64 = 4;
const SOCKET_INODE: i64 = 5;
const PROCESS_ROWS_MAX_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessRowSpan {
    pub pid: u32,
    pub token_start: usize,
    pub token_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessTokenSpan {
    pub start: usize,
    pub end: usize,
}

unsafe extern "C" {
    fn prodex_status_resource_parser_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        auxiliary_address: u64,
        auxiliary_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_status_process_rows_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        row_output_address: u64,
        row_capacity: i64,
        token_output_address: u64,
        token_capacity: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

fn call_with_aux_length(
    operation: i64,
    input: &[u8],
    auxiliary: &[u8],
    auxiliary_length: usize,
) -> Result<[i64; 3], MojoError> {
    let mut output = [0_i64; 3];
    status(unsafe {
        prodex_status_resource_parser_v1(
            ABI_VERSION,
            operation,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            auxiliary.as_ptr() as usize as u64,
            i64::try_from(auxiliary_length).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

fn call(operation: i64, input: &[u8], auxiliary: &[u8]) -> Result<[i64; 3], MojoError> {
    call_with_aux_length(operation, input, auxiliary, auxiliary.len())
}

fn parse_scalar(operation: i64, input: &str) -> Option<u64> {
    let output = call(operation, input.as_bytes(), &[]).ok()?;
    if output[0] != 1 || output[1] < 0 {
        return None;
    }
    u64::try_from(output[1]).ok()
}

pub fn parse_process_cpu_ticks(text: &str) -> Option<u64> {
    parse_scalar(PROCESS_CPU, text)
}

pub fn parse_system_cpu_ticks(text: &str) -> Option<u64> {
    parse_scalar(SYSTEM_CPU, text)
}

fn parse_field(text: &str, key: &str, operation: i64) -> Option<u64> {
    let output = call(operation, text.as_bytes(), key.as_bytes()).ok()?;
    if output[0] != 1 || output[1] < 0 {
        return None;
    }
    u64::try_from(output[1]).ok()
}

pub fn parse_kib_field(text: &str, key: &str) -> Option<u64> {
    parse_field(text, key, KIB_FIELD)
}

pub fn parse_u64_field(text: &str, key: &str) -> Option<u64> {
    parse_field(text, key, U64_FIELD)
}

pub fn parse_network_queues(text: &str, inodes: &HashSet<u64>) -> (u64, u64) {
    let inode_values = inodes.iter().copied().collect::<Vec<_>>();
    let auxiliary = unsafe {
        std::slice::from_raw_parts(
            inode_values.as_ptr().cast::<u8>(),
            inode_values.len() * std::mem::size_of::<u64>(),
        )
    };
    let output = call_with_aux_length(
        NETWORK_QUEUES,
        text.as_bytes(),
        auxiliary,
        inode_values.len(),
    );
    let Ok(output) = output else {
        return (0, 0);
    };
    if output[0] != 1 || output[1] < 0 || output[2] < 0 {
        return (0, 0);
    }
    (
        u64::try_from(output[1]).unwrap_or_default(),
        u64::try_from(output[2]).unwrap_or_default(),
    )
}

pub fn socket_inode_target(target: &str) -> Option<u64> {
    parse_scalar(SOCKET_INODE, target)
}

fn process_rows_call(
    text: &[u8],
    rows: &mut [i64],
    tokens: &mut [i64],
    output: &mut [i64; 2],
) -> Result<(), MojoError> {
    let row_capacity = rows.len() / 3;
    let token_capacity = tokens.len() / 2;
    status(unsafe {
        prodex_status_process_rows_v1(
            ABI_VERSION,
            pointer_address(text.as_ptr()),
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            if rows.is_empty() {
                0
            } else {
                mutable_pointer_address(rows.as_mut_ptr())
            },
            i64::try_from(row_capacity).map_err(|_| MojoError::InvalidInput)?,
            if tokens.is_empty() {
                0
            } else {
                mutable_pointer_address(tokens.as_mut_ptr())
            },
            i64::try_from(token_capacity).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })
}

/// Parses bounded `ps -Ao pid=,comm=,args=` text into caller-owned byte spans.
///
/// Mojo owns line/token acceptance and PID parsing; Rust only validates spans and
/// materializes `&str` slices for its process-row adapter.
pub fn parse_process_rows(
    text: &str,
) -> Result<(Vec<ProcessRowSpan>, Vec<ProcessTokenSpan>), MojoError> {
    if text.len() > PROCESS_ROWS_MAX_INPUT_BYTES {
        return Err(MojoError::Capacity);
    }
    let bytes = text.as_bytes();
    let mut counts = [0_i64; 2];
    process_rows_call(bytes, &mut [], &mut [], &mut counts)?;
    let row_count = usize::try_from(counts[0]).map_err(|_| MojoError::InvalidOutput)?;
    let token_count = usize::try_from(counts[1]).map_err(|_| MojoError::InvalidOutput)?;
    let mut rows = vec![0_i64; row_count.checked_mul(3).ok_or(MojoError::InvalidOutput)?];
    let mut tokens = vec![0_i64; token_count.checked_mul(2).ok_or(MojoError::InvalidOutput)?];
    let mut written = [0_i64; 2];
    process_rows_call(bytes, &mut rows, &mut tokens, &mut written)?;
    if written != counts {
        return Err(MojoError::InvalidOutput);
    }

    let mut parsed_rows = Vec::with_capacity(row_count);
    let mut parsed_tokens = Vec::with_capacity(token_count);
    for chunk in rows.as_chunks::<3>().0 {
        let pid = u32::try_from(chunk[0]).map_err(|_| MojoError::InvalidOutput)?;
        let token_start = usize::try_from(chunk[1]).map_err(|_| MojoError::InvalidOutput)?;
        let token_count = usize::try_from(chunk[2]).map_err(|_| MojoError::InvalidOutput)?;
        if token_count < 2
            || token_start > tokens.len() / 2
            || token_count > tokens.len() / 2 - token_start
        {
            return Err(MojoError::InvalidOutput);
        }
        parsed_rows.push(ProcessRowSpan {
            pid,
            token_start,
            token_count,
        });
    }
    for chunk in tokens.as_chunks::<2>().0 {
        let start = usize::try_from(chunk[0]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(chunk[1]).map_err(|_| MojoError::InvalidOutput)?;
        if start >= end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(MojoError::InvalidOutput);
        }
        parsed_tokens.push(ProcessTokenSpan { start, end });
    }
    let mut token_cursor = 0;
    for row in &parsed_rows {
        if row.token_start != token_cursor
            || row.token_count > parsed_tokens.len().saturating_sub(token_cursor)
        {
            return Err(MojoError::InvalidOutput);
        }
        token_cursor += row.token_count;
    }
    if token_cursor != parsed_tokens.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok((parsed_rows, parsed_tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_preserve_proc_contracts() {
        assert_eq!(
            parse_process_cpu_ticks("123 (prodex worker) S 1 2 3 4 5 6 7 8 9 10 120 30 0 0 0"),
            Some(150)
        );
        assert_eq!(
            parse_system_cpu_ticks("cpu  10 20 30 40 50 60 70 80 90\ncpu0 1 2 3 4"),
            Some(360)
        );
        assert_eq!(
            parse_kib_field("VmRSS: 2048 kB\n", "VmRSS"),
            Some(2_097_152)
        );
        assert_eq!(
            parse_u64_field("read_bytes: 123\nwrite_bytes: 456\n", "write_bytes"),
            Some(456)
        );
    }

    #[test]
    fn network_parser_filters_inode_and_decodes_hex_queues() {
        let table = concat!(
            "sl local_address rem_address st tx_queue:rx_queue tr tm->when retrnsmt uid timeout inode\n",
            "0: 0100007F:1F90 00000000:0000 0A 00000010:00000020 00:00000000 00000000 1000 0 42\n",
            "1: 0100007F:1F91 00000000:0000 0A 00000100:00000200 00:00000000 00000000 1000 0 99\n",
        );
        assert_eq!(
            parse_network_queues(table, &HashSet::from([42])),
            (0x20, 0x10)
        );
    }

    #[test]
    fn socket_target_requires_exact_proc_syntax() {
        assert_eq!(socket_inode_target("socket:[42]"), Some(42));
        assert_eq!(socket_inode_target("socket:[42] trailing"), None);
        assert_eq!(socket_inode_target("pipe:[42]"), None);
    }

    #[test]
    fn process_rows_parser_returns_ordered_spans_and_rejects_oversized_input() {
        let text =
            "  111 prodex /usr/local/bin/prodex run --profile main\ninvalid\n222 bash bash\n";
        let (rows, tokens) =
            parse_process_rows(text).expect("compiled Mojo parser should accept ps text");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pid, 111);
        assert_eq!(rows[0].token_count, 6);
        assert_eq!(&text[tokens[1].start..tokens[1].end], "prodex");
        assert_eq!(&text[tokens[5].start..tokens[5].end], "main");
        assert!(matches!(
            parse_process_rows(&"x".repeat(PROCESS_ROWS_MAX_INPUT_BYTES + 1)),
            Err(MojoError::Capacity)
        ));
    }
}
