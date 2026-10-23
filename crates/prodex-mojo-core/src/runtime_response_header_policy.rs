//! Typed host bridge for compiled Mojo HTTP Connection headers.
//! Rust retains source buffers and ordered output; Mojo owns filtering and tokens.

use crate::MojoError;

mod request_tokens;
pub use request_tokens::request_connection_tokens;

#[repr(C)]
#[derive(Clone, Copy)]
struct HeaderByteView {
    address: u64,
    length: u64,
}

unsafe extern "C" {
    fn prodex_runtime_response_forwarding_header_batch_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        connection_values: u64,
        value_count: i64,
    ) -> i64;
}

/// One compiled-Mojo decision, regardless of how many upstream Connection
/// header values were provided. No Rust token parser or fallback.
pub fn should_forward(name: &str, connection_values: &[&[u8]]) -> Result<bool, MojoError> {
    let values = connection_values
        .iter()
        .map(|value| {
            Ok(HeaderByteView {
                address: value.as_ptr() as u64,
                length: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outcome = unsafe {
        prodex_runtime_response_forwarding_header_batch_v1(
            1,
            name.as_ptr() as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match outcome {
        0 => Ok(true),
        1..=3 => Ok(false),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_connection_headers_filter_hop_headers_and_borrowed_tokens() {
        let connection: [&[u8]; 3] = [
            b"  keep-alive, X-First, x-second ",
            b"upgrade, X-THIRD",
            b"   ",
        ];
        for name in [
            "connection",
            "Content-Length",
            "transfer-encoding",
            "TE",
            "X-First",
            "x-second",
            "X-third",
        ] {
            assert_eq!(should_forward(name, &connection), Ok(false), "{name}");
        }
        for name in ["x-codex-turn-state", "Content-Type", "Date", "x-ignore"] {
            assert_eq!(should_forward(name, &connection), Ok(true), "{name}");
        }
        assert_eq!(should_forward("X-First", &[]), Ok(true));
        assert_eq!(should_forward("X-First", &[b"\xff\xfe"]), Ok(true));
        assert_eq!(
            should_forward("X-First", &[b"X-First,", b" X-THIRD"]),
            Ok(false)
        );
    }

    #[test]
    fn batch_connection_header_abi_rejects_untrusted_lengths_and_tags() {
        let name = "X-First";
        let values = [HeaderByteView {
            address: b"a".as_ptr() as u64,
            length: 1,
        }];
        unsafe {
            assert_eq!(
                prodex_runtime_response_forwarding_header_batch_v1(
                    2,
                    name.as_ptr() as u64,
                    name.len() as i64,
                    values.as_ptr() as u64,
                    1,
                ),
                -4
            );
            assert_eq!(
                prodex_runtime_response_forwarding_header_batch_v1(
                    1,
                    0,
                    name.len() as i64,
                    values.as_ptr() as u64,
                    1,
                ),
                -1
            );
            assert_eq!(
                prodex_runtime_response_forwarding_header_batch_v1(
                    1,
                    name.as_ptr() as u64,
                    name.len() as i64,
                    0,
                    1,
                ),
                -1
            );
            assert_eq!(
                prodex_runtime_response_forwarding_header_batch_v1(
                    1,
                    name.as_ptr() as u64,
                    name.len() as i64,
                    values.as_ptr() as u64,
                    -1,
                ),
                -1
            );
        }
    }
}
