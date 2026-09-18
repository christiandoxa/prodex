pub const ABI_VERSION: u32 = 1;

unsafe extern "C" {
    fn prodex_mojo_abi_version() -> i64;
}

pub fn abi_version() -> Result<u32, crate::MojoError> {
    let version = u32::try_from(unsafe { prodex_mojo_abi_version() })
        .map_err(|_| crate::MojoError::AbiMismatch)?;
    (version == ABI_VERSION)
        .then_some(version)
        .ok_or(crate::MojoError::AbiMismatch)
}

pub fn self_test() -> bool {
    abi_version() == Ok(ABI_VERSION)
}
