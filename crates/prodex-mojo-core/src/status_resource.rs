use crate::MojoError;
use std::collections::HashSet;

const ABI_VERSION: i64 = 1;
const PROCESS_CPU: i64 = 0;
const SYSTEM_CPU: i64 = 1;
const KIB_FIELD: i64 = 2;
const U64_FIELD: i64 = 3;
const NETWORK_QUEUES: i64 = 4;
const SOCKET_INODE: i64 = 5;

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
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
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
}
