use crate::MojoError;

const GEMINI_REQUEST_CONTENT_ABI_VERSION: i64 = 1;
const GEMINI_REQUEST_CONTENT_STATUS_INVALID: i64 = 1;
const GEMINI_REQUEST_CONTENT_STATUS_CAPACITY: i64 = 2;
const GEMINI_REQUEST_CONTENT_STATUS_ABI_MISMATCH: i64 = 3;
const GEMINI_REQUEST_CONTENT_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Deterministic JSON-shaping operations for Gemini request translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiRequestContentOperation {
    SanitizeSchema = 1,
    SanitizeFunctionSchema = 2,
    Content = 3,
    SystemInstruction = 4,
    TextPart = 5,
    FunctionCallPart = 6,
    FunctionResponsePart = 7,
    ToolDeclaration = 8,
    ToolConfig = 9,
    BuiltinTool = 10,
}

impl GeminiRequestContentOperation {
    const fn code(self) -> i64 {
        self as i64
    }
}

/// Borrowed JSON or JSON-string inputs for one bounded Gemini request operation.
#[derive(Debug, Clone, Copy)]
pub struct GeminiRequestContentKernelInput<'a> {
    pub operation: GeminiRequestContentOperation,
    pub primary: Option<&'a [u8]>,
    pub secondary: Option<&'a [u8]>,
    pub tertiary: Option<&'a [u8]>,
    pub quaternary: Option<&'a [u8]>,
    pub kind: i64,
}

impl<'a> GeminiRequestContentKernelInput<'a> {
    pub const fn new(operation: GeminiRequestContentOperation) -> Self {
        Self {
            operation,
            primary: None,
            secondary: None,
            tertiary: None,
            quaternary: None,
            kind: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiRequestContentStringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiRequestContentFfiInput {
    operation: i64,
    primary: GeminiRequestContentStringView,
    secondary: GeminiRequestContentStringView,
    tertiary: GeminiRequestContentStringView,
    quaternary: GeminiRequestContentStringView,
    primary_present: i64,
    secondary_present: i64,
    tertiary_present: i64,
    quaternary_present: i64,
    kind: i64,
}

const _: () = assert!(std::mem::size_of::<GeminiRequestContentStringView>() == 16);
const _: () = assert!(std::mem::size_of::<GeminiRequestContentFfiInput>() == 112);

unsafe extern "C" {
    fn prodex_gemini_request_content_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

fn view(value: Option<&[u8]>) -> GeminiRequestContentStringView {
    value
        .map(|value| GeminiRequestContentStringView {
            ptr: pointer_address(value.as_ptr()),
            len: value.len() as u64,
        })
        .unwrap_or_default()
}

fn output_capacity(input: &GeminiRequestContentKernelInput<'_>) -> Result<usize, MojoError> {
    let total = [
        input.primary,
        input.secondary,
        input.tertiary,
        input.quaternary,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        if value.len() > GEMINI_REQUEST_CONTENT_MAX_BYTES {
            return Err(MojoError::InvalidInput);
        }
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })?;
    let multiplier = match input.operation {
        GeminiRequestContentOperation::SanitizeSchema
        | GeminiRequestContentOperation::SanitizeFunctionSchema => 8,
        _ => 3,
    };
    total
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(1024))
        .ok_or(MojoError::InvalidInput)
}

/// Runs one bounded, caller-owned Gemini request-content operation in Mojo.
pub fn gemini_request_content_kernel(
    input: GeminiRequestContentKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    let capacity = output_capacity(&input)?;
    let mut output = vec![0_u8; capacity];
    let ffi_input = GeminiRequestContentFfiInput {
        operation: input.operation.code(),
        primary: view(input.primary),
        secondary: view(input.secondary),
        tertiary: view(input.tertiary),
        quaternary: view(input.quaternary),
        primary_present: i64::from(input.primary.is_some()),
        secondary_present: i64::from(input.secondary.is_some()),
        tertiary_present: i64::from(input.tertiary.is_some()),
        quaternary_present: i64::from(input.quaternary.is_some()),
        kind: input.kind,
    };
    let mut written = 0_i64;
    let status = unsafe {
        prodex_gemini_request_content_kernel_v1(
            GEMINI_REQUEST_CONTENT_ABI_VERSION,
            pointer_address(&ffi_input),
            mutable_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut written),
        )
    };
    match status {
        0 => {}
        GEMINI_REQUEST_CONTENT_STATUS_INVALID => return Err(MojoError::InvalidInput),
        GEMINI_REQUEST_CONTENT_STATUS_CAPACITY => return Err(MojoError::Capacity),
        GEMINI_REQUEST_CONTENT_STATUS_ABI_MISMATCH => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}

#[cfg(all(test, feature = "mojo-provider-constraints"))]
mod tests {
    use super::*;

    #[test]
    fn schema_kernel_preserves_json_boundary_and_removes_gemini_unsupported_keys() {
        let schema = br#"{"type":"object","strict":true,"properties":{"id":{"type":"string","additionalProperties":false}}}"#;
        let mut input =
            GeminiRequestContentKernelInput::new(GeminiRequestContentOperation::SanitizeSchema);
        input.primary = Some(schema);
        let output = gemini_request_content_kernel(input).unwrap();
        assert_eq!(
            output,
            br#"{"type":"object","properties":{"id":{"type":"string"}}}"#
        );
    }
}
