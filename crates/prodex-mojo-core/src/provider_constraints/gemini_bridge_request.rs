use crate::MojoError;

const GEMINI_BRIDGE_REQUEST_ABI_VERSION: i64 = 1;
const GEMINI_BRIDGE_REQUEST_STATUS_INVALID: i64 = 1;
const GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY: i64 = 2;
const GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH: i64 = 3;
const GEMINI_BRIDGE_REQUEST_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Deterministic request operations that remain after the Gemini content kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiBridgeRequestOperation {
    GenerateContentRequest = 1,
    GenerateContentBody = 2,
    GenerationConfig = 3,
    NativeProject = 4,
    RequestBodyWithoutTool = 5,
    SimpleRequest = 6,
    ValidateCandidateCount = 7,
}

impl GeminiBridgeRequestOperation {
    const fn code(self) -> i64 {
        self as i64
    }
}

/// Borrowed JSON fragments for one bounded Gemini bridge request operation.
#[derive(Debug, Clone, Copy)]
pub struct GeminiBridgeRequestKernelInput<'a> {
    pub operation: GeminiBridgeRequestOperation,
    pub primary: Option<&'a [u8]>,
    pub secondary: Option<&'a [u8]>,
    pub tertiary: Option<&'a [u8]>,
    pub quaternary: Option<&'a [u8]>,
    pub quinary: Option<&'a [u8]>,
    pub senary: Option<&'a [u8]>,
    pub septenary: Option<&'a [u8]>,
    pub octonary: Option<&'a [u8]>,
    pub kind: i64,
}

impl<'a> GeminiBridgeRequestKernelInput<'a> {
    pub const fn new(operation: GeminiBridgeRequestOperation) -> Self {
        Self {
            operation,
            primary: None,
            secondary: None,
            tertiary: None,
            quaternary: None,
            quinary: None,
            senary: None,
            septenary: None,
            octonary: None,
            kind: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiBridgeRequestStringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiBridgeRequestFfiInput {
    operation: i64,
    primary: GeminiBridgeRequestStringView,
    secondary: GeminiBridgeRequestStringView,
    tertiary: GeminiBridgeRequestStringView,
    quaternary: GeminiBridgeRequestStringView,
    quinary: GeminiBridgeRequestStringView,
    senary: GeminiBridgeRequestStringView,
    septenary: GeminiBridgeRequestStringView,
    octonary: GeminiBridgeRequestStringView,
    primary_present: i64,
    secondary_present: i64,
    tertiary_present: i64,
    quaternary_present: i64,
    quinary_present: i64,
    senary_present: i64,
    septenary_present: i64,
    octonary_present: i64,
    kind: i64,
}

const _: () = assert!(std::mem::size_of::<GeminiBridgeRequestStringView>() == 16);
const _: () = assert!(std::mem::size_of::<GeminiBridgeRequestFfiInput>() == 208);

unsafe extern "C" {
    fn prodex_gemini_bridge_request_kernel_v1(
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

fn view(value: Option<&[u8]>) -> GeminiBridgeRequestStringView {
    value
        .map(|value| GeminiBridgeRequestStringView {
            ptr: pointer_address(value.as_ptr()),
            len: value.len() as u64,
        })
        .unwrap_or_default()
}

fn output_capacity(input: &GeminiBridgeRequestKernelInput<'_>) -> Result<usize, MojoError> {
    [
        input.primary,
        input.secondary,
        input.tertiary,
        input.quaternary,
        input.quinary,
        input.senary,
        input.septenary,
        input.octonary,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        if value.len() > GEMINI_BRIDGE_REQUEST_MAX_BYTES {
            return Err(MojoError::InvalidInput);
        }
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
    .and_then(|total| {
        total
            .checked_mul(4)
            .and_then(|value| value.checked_add(4096))
            .ok_or(MojoError::InvalidInput)
    })
}

/// Runs one bounded, caller-owned Gemini bridge request operation in Mojo.
pub fn gemini_bridge_request_kernel(
    input: GeminiBridgeRequestKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    let capacity = output_capacity(&input)?;
    let mut output = vec![0_u8; capacity];
    let ffi_input = GeminiBridgeRequestFfiInput {
        operation: input.operation.code(),
        primary: view(input.primary),
        secondary: view(input.secondary),
        tertiary: view(input.tertiary),
        quaternary: view(input.quaternary),
        quinary: view(input.quinary),
        senary: view(input.senary),
        septenary: view(input.septenary),
        octonary: view(input.octonary),
        primary_present: i64::from(input.primary.is_some()),
        secondary_present: i64::from(input.secondary.is_some()),
        tertiary_present: i64::from(input.tertiary.is_some()),
        quaternary_present: i64::from(input.quaternary.is_some()),
        quinary_present: i64::from(input.quinary.is_some()),
        senary_present: i64::from(input.senary.is_some()),
        septenary_present: i64::from(input.septenary.is_some()),
        octonary_present: i64::from(input.octonary.is_some()),
        kind: input.kind,
    };
    let mut written = 0_i64;
    let status = unsafe {
        prodex_gemini_bridge_request_kernel_v1(
            GEMINI_BRIDGE_REQUEST_ABI_VERSION,
            pointer_address(&ffi_input),
            mutable_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut written),
        )
    };
    match status {
        0 => {}
        GEMINI_BRIDGE_REQUEST_STATUS_INVALID => return Err(MojoError::InvalidInput),
        GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY => return Err(MojoError::Capacity),
        GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}
