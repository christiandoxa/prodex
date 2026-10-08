//! Thin typed ABI for Mojo-owned Gemini thought-signature precedence.

use crate::MojoError;

/// One present (possibly non-string) response JSON field, independent of priority.
#[derive(Debug, Clone, Copy)]
pub struct GeminiSignatureCandidate<'a> {
    pub present: bool,
    pub text: Option<&'a str>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct GeminiSignatureField {
    present: i64,
    is_string: i64,
    text_address: u64,
    text_length: u64,
}

const _: () = assert!(std::mem::size_of::<GeminiSignatureField>() == 32);
const GEMINI_SIGNATURE_CHOICE_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_gemini_signature_choice_v1(
        version: i64,
        fields_address: u64,
        selected_index_address: u64,
    ) -> i64;
}

/// Chooses the first present field by protocol precedence, then tests its string
/// shape/Unicode whitespace in Mojo. Rejected present fields do not fall through.
pub fn gemini_signature_choice(
    candidates: &[GeminiSignatureCandidate<'_>; 7],
) -> Result<Option<usize>, MojoError> {
    let mut fields = [GeminiSignatureField::default(); 7];
    for (field, candidate) in fields.iter_mut().zip(candidates) {
        field.present = i64::from(candidate.present);
        field.is_string = i64::from(candidate.text.is_some());
        if let Some(value) = candidate.text {
            field.text_address = value.as_ptr() as usize as u64;
            field.text_length = u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
        }
    }
    let mut selected = -1_i64;
    let status = unsafe {
        prodex_gemini_signature_choice_v1(
            GEMINI_SIGNATURE_CHOICE_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            (&mut selected as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(if status == 4 {
            MojoError::AbiMismatch
        } else {
            MojoError::InvalidOutput
        });
    }
    match selected {
        -1 => Ok(None),
        0..=6 => Ok(Some(selected as usize)),
        _ => Err(MojoError::InvalidOutput),
    }
}
