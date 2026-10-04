use crate::MojoError;

const GEMINI_INTERNAL_INSTRUCTION_ABI_VERSION: i64 = 1;
const LEAK_TEXT: i64 = 1;
const SANITIZE_TEXT: i64 = 2;
const NORMALIZE_CORPUS: i64 = 3;
const TEXT_ECHO: i64 = 4;

unsafe extern "C" {
    fn prodex_mojo_gemini_internal_instruction_v1(
        abi_version: i64,
        operation: i64,
        text_address: u64,
        text_length: i64,
        corpus_address: u64,
        corpus_length: i64,
        output_address: u64,
        output_capacity: i64,
        decision_address: u64,
        output_length_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn call(
    operation: i64,
    text: &str,
    corpus: &str,
    output: &mut [u8],
) -> Result<(i64, usize), MojoError> {
    let text_length = signed_len(text)?;
    let corpus_length = signed_len(corpus)?;
    let output_capacity = i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut decision = 0_i64;
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_internal_instruction_v1(
            GEMINI_INTERNAL_INSTRUCTION_ABI_VERSION,
            operation,
            text.as_ptr() as usize as u64,
            text_length,
            corpus.as_ptr() as usize as u64,
            corpus_length,
            output.as_mut_ptr() as usize as u64,
            output_capacity,
            (&mut decision as *mut i64) as usize as u64,
            (&mut output_length as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::AbiMismatch),
        3 => return Err(MojoError::Capacity),
        _ => return Err(MojoError::InvalidOutput),
    }
    let output_length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if output_length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok((decision, output_length))
}

pub fn leak_text(text: &str) -> Result<bool, MojoError> {
    let (decision, output_length) = call(LEAK_TEXT, text, "", &mut [])?;
    if output_length != 0 {
        return Err(MojoError::InvalidOutput);
    }
    match decision {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn sanitize_text(text: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; text.len()];
    let (decision, output_length) = call(SANITIZE_TEXT, text, "", &mut output)?;
    match decision {
        0 | 1 => {
            output.truncate(output_length);
            String::from_utf8(output)
                .map(Some)
                .map_err(|_| MojoError::InvalidOutput)
        }
        2 if output_length == 0 => Ok(None),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn normalize_corpus(text: &str) -> Result<String, MojoError> {
    let mut output = vec![0_u8; text.len()];
    let (decision, output_length) = call(NORMALIZE_CORPUS, text, "", &mut output)?;
    if decision != 0 {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(output_length);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

pub fn text_echoes(text: &str, corpus: &str) -> Result<bool, MojoError> {
    let (decision, output_length) = call(TEXT_ECHO, text, corpus, &mut [])?;
    if output_length != 0 {
        return Err(MojoError::InvalidOutput);
    }
    match decision {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
