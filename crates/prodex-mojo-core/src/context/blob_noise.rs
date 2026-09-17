#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBlobNoiseAnalysis {
    pub line_flags: Vec<i64>,
    pub primary_binary: bool,
    pub binary: Option<ContextBlobBinaryFinding>,
    pub base64: Option<ContextBlobBase64Finding>,
    pub minified: Option<ContextBlobMinifiedFinding>,
    pub lock: Option<ContextBlobLockFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBlobBinaryFinding {
    pub first_line: usize,
    pub score: usize,
    pub suspicious: usize,
    pub nul: usize,
    pub replacement: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBlobBase64Finding {
    pub line: usize,
    pub bytes: usize,
    pub score: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBlobMinifiedFinding {
    pub kind: i64,
    pub score: usize,
    pub max_line_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBlobLockFinding {
    pub kind: i64,
    pub score: usize,
    pub line: usize,
    pub value: usize,
}

pub const CONTEXT_BLOB_PRIMARY_BASE64: i64 = 1;
pub const CONTEXT_BLOB_PRIMARY_MINIFIED: i64 = 2;
const CONTEXT_BLOB_NOISE_OUTPUT_FIELDS: usize = 22;
const CONTEXT_BLOB_NOISE_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_context_blob_noise_analyze_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        normalized_address: u64,
        normalized_length: i64,
        line_flags_address: u64,
        line_count: i64,
        output_address: u64,
        output_count: i64,
    ) -> i64;
}

fn usize_field(value: i64) -> Result<usize, crate::MojoError> {
    usize::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)
}

fn positive_line(value: i64) -> Result<usize, crate::MojoError> {
    let value = usize_field(value)?;
    (value > 0)
        .then_some(value)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn analyze_blob_noise(
    input: &str,
    normalized: &str,
    line_count: usize,
) -> Result<ContextBlobNoiseAnalysis, crate::MojoError> {
    let mut flags = vec![0_i64; line_count];
    let mut output = [0_i64; CONTEXT_BLOB_NOISE_OUTPUT_FIELDS];
    let status = unsafe {
        prodex_context_blob_noise_analyze_v1(
            CONTEXT_BLOB_NOISE_ABI_VERSION,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            normalized.as_ptr() as usize as u64,
            i64::try_from(normalized.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            flags.as_mut_ptr() as usize as u64,
            i64::try_from(line_count).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            CONTEXT_BLOB_NOISE_OUTPUT_FIELDS as i64,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        2 => return Err(crate::MojoError::InvalidOutput),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    if output[0] != CONTEXT_BLOB_NOISE_ABI_VERSION
        || flags.iter().any(|flag| flag & !3 != 0)
        || !matches!(output[1], 0 | 1)
        || !matches!(output[2], 0 | 1)
        || !matches!(output[8], 0 | 1)
        || !matches!(output[12], 0 | 1)
        || !matches!(output[16], 0 | 1)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let binary = (output[2] == 1)
        .then(|| {
            Ok(ContextBlobBinaryFinding {
                first_line: positive_line(output[3])?,
                score: usize_field(output[4])?,
                suspicious: usize_field(output[5])?,
                nul: usize_field(output[6])?,
                replacement: usize_field(output[7])?,
            })
        })
        .transpose()?;
    let base64 = (output[8] == 1)
        .then(|| {
            Ok(ContextBlobBase64Finding {
                line: positive_line(output[9])?,
                bytes: usize_field(output[10])?,
                score: usize_field(output[11])?,
            })
        })
        .transpose()?;
    let minified = (output[12] == 1)
        .then(|| {
            if !matches!(output[13], 1 | 2) {
                return Err(crate::MojoError::InvalidOutput);
            }
            Ok(ContextBlobMinifiedFinding {
                kind: output[13],
                score: usize_field(output[14])?,
                max_line_bytes: usize_field(output[15])?,
            })
        })
        .transpose()?;
    let lock = (output[16] == 1)
        .then(|| {
            if !(1..=4).contains(&output[17]) {
                return Err(crate::MojoError::InvalidOutput);
            }
            Ok(ContextBlobLockFinding {
                kind: output[17],
                score: usize_field(output[18])?,
                line: positive_line(output[19])?,
                value: usize_field(output[20])?,
            })
        })
        .transpose()?;
    Ok(ContextBlobNoiseAnalysis {
        line_flags: flags,
        primary_binary: output[1] == 1,
        binary,
        base64,
        minified,
        lock,
    })
}
