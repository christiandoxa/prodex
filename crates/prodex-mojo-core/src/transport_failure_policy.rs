use crate::MojoError;
use std::sync::OnceLock;

const ABI_VERSION: i64 = 1;
const TEXT_LABEL: i64 = 0;
const TEXT_MARKER: i64 = 1;
const FAILURE_KIND_COUNT: usize = 11;

unsafe extern "C" {
    fn prodex_transport_failure_text_v1(
        abi_version: i64,
        operation: i64,
        present: i64,
        kind: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_transport_failure_classify_message_v1(
        abi_version: i64,
        address: u64,
        length: i64,
    ) -> i64;
    fn prodex_transport_failure_health_penalty_v1(abi_version: i64, kind: i64) -> i64;
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn load_text(operation: i64, present: bool, kind: i64) -> Result<String, MojoError> {
    let mut output = [0_u8; 64];
    let mut written = -1_i64;
    status(unsafe {
        prodex_transport_failure_text_v1(
            ABI_VERSION,
            operation,
            i64::from(present),
            kind,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn failure_kind_label(kind: i64) -> Result<&'static str, MojoError> {
    static LABELS: OnceLock<Result<Vec<String>, MojoError>> = OnceLock::new();
    let index = usize::try_from(kind)
        .ok()
        .filter(|index| *index < FAILURE_KIND_COUNT)
        .ok_or(MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (0_i64..FAILURE_KIND_COUNT as i64)
            .map(|kind| load_text(TEXT_LABEL, true, kind))
            .collect()
    }) {
        Ok(labels) => labels
            .get(index)
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn upstream_connect_failure_marker(kind: Option<i64>) -> Result<&'static str, MojoError> {
    static MARKERS: OnceLock<Result<Vec<String>, MojoError>> = OnceLock::new();
    let index = match kind {
        None => 0,
        Some(kind) => usize::try_from(kind)
            .ok()
            .filter(|index| *index < FAILURE_KIND_COUNT)
            .map(|index| index + 1)
            .ok_or(MojoError::InvalidInput)?,
    };
    match MARKERS.get_or_init(|| {
        let mut markers = Vec::with_capacity(FAILURE_KIND_COUNT + 1);
        markers.push(load_text(TEXT_MARKER, false, 0)?);
        for kind in 0_i64..FAILURE_KIND_COUNT as i64 {
            markers.push(load_text(TEXT_MARKER, true, kind)?);
        }
        Ok(markers)
    }) {
        Ok(markers) => markers
            .get(index)
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn classify_message(message: &str) -> Result<Option<i64>, MojoError> {
    let result = unsafe {
        prodex_transport_failure_classify_message_v1(
            ABI_VERSION,
            message.as_ptr() as usize as u64,
            i64::try_from(message.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        -3 => Err(MojoError::AbiMismatch),
        -2 => Err(MojoError::InvalidInput),
        -1 => Ok(None),
        0..=10 => Ok(Some(result)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn health_penalty(kind: i64) -> Result<u32, MojoError> {
    let result = unsafe { prodex_transport_failure_health_penalty_v1(ABI_VERSION, kind) };
    match result {
        -3 => Err(MojoError::AbiMismatch),
        -2 => Err(MojoError::InvalidInput),
        4 | 5 => u32::try_from(result).map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_failure_policy_preserves_labels_classification_and_penalties() {
        assert_eq!(failure_kind_label(0).unwrap(), "dns");
        assert_eq!(
            failure_kind_label(9).unwrap(),
            "upstream_closed_before_commit"
        );
        assert_eq!(
            upstream_connect_failure_marker(Some(1)).unwrap(),
            "upstream_connect_timeout"
        );
        assert_eq!(
            upstream_connect_failure_marker(Some(8)).unwrap(),
            "upstream_connect_timeout"
        );
        assert_eq!(
            upstream_connect_failure_marker(None).unwrap(),
            "upstream_connect_error"
        );

        assert_eq!(
            classify_message("FAILED TO LOOKUP ADDRESS INFORMATION").unwrap(),
            Some(0)
        );
        assert_eq!(classify_message("TLS handshake failed").unwrap(), Some(4));
        assert_eq!(
            classify_message("stream closed before response.completed").unwrap(),
            Some(9)
        );
        assert_eq!(classify_message("not a transport failure").unwrap(), None);

        assert_eq!(health_penalty(0).unwrap(), 5);
        assert_eq!(health_penalty(4).unwrap(), 5);
        assert_eq!(health_penalty(6).unwrap(), 4);
        assert_eq!(health_penalty(10).unwrap(), 4);
    }
}
