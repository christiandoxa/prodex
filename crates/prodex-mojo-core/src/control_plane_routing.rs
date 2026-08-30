use crate::MojoError;

pub const ABI_VERSION: i64 = 1;
pub const CONTROL_PLANE_OPERATION_COUNT: usize = 35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPlaneOperationTag(u8);

impl ControlPlaneOperationTag {
    pub fn new(value: usize) -> Option<Self> {
        (value < CONTROL_PLANE_OPERATION_COUNT).then_some(Self(value as u8))
    }

    fn raw(self) -> i64 {
        i64::from(self.0)
    }
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPlaneHttpMethod {
    Get = 0,
    Post = 1,
    Put = 2,
    Patch = 3,
    Delete = 4,
    Options = 5,
    Other = 6,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPlaneRouteValidationMode {
    Exact = 0,
    AllowAlias = 1,
    AllowAliasAndMethodCheck = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPlaneRouteValidationInput {
    pub route_operation: ControlPlaneOperationTag,
    pub action_operation: ControlPlaneOperationTag,
    pub method: ControlPlaneHttpMethod,
    pub mode: ControlPlaneRouteValidationMode,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPlaneRouteValidationDecision {
    Allow = 0,
    OperationMismatch = 1,
    MethodNotAllowed = 2,
}

unsafe extern "C" {
    fn prodex_mojo_control_plane_route_validation_v1(
        abi_version: i64,
        route_operation: i64,
        action_operation: i64,
        method: i64,
        mode: i64,
        decision_address: u64,
    ) -> i64;
}

pub fn validate(
    input: ControlPlaneRouteValidationInput,
) -> Result<ControlPlaneRouteValidationDecision, MojoError> {
    let mut decision = -1_i64;
    let status = unsafe {
        prodex_mojo_control_plane_route_validation_v1(
            ABI_VERSION,
            input.route_operation.raw(),
            input.action_operation.raw(),
            input.method as i64,
            input.mode as i64,
            pointer_address(&mut decision),
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    match decision {
        0 => Ok(ControlPlaneRouteValidationDecision::Allow),
        1 => Ok(ControlPlaneRouteValidationDecision::OperationMismatch),
        2 => Ok(ControlPlaneRouteValidationDecision::MethodNotAllowed),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    let tag = |value| ControlPlaneOperationTag::new(value).expect("fixed operation tag");
    let alias = validate(ControlPlaneRouteValidationInput {
        route_operation: tag(14),
        action_operation: tag(16),
        method: ControlPlaneHttpMethod::Patch,
        mode: ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck,
    });
    let method = validate(ControlPlaneRouteValidationInput {
        route_operation: tag(12),
        action_operation: tag(16),
        method: ControlPlaneHttpMethod::Get,
        mode: ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck,
    });
    let mismatch = validate(ControlPlaneRouteValidationInput {
        route_operation: tag(14),
        action_operation: tag(16),
        method: ControlPlaneHttpMethod::Patch,
        mode: ControlPlaneRouteValidationMode::Exact,
    });
    alias == Ok(ControlPlaneRouteValidationDecision::Allow)
        && method == Ok(ControlPlaneRouteValidationDecision::MethodNotAllowed)
        && mismatch == Ok(ControlPlaneRouteValidationDecision::OperationMismatch)
}

#[inline]
fn pointer_address(pointer: *mut i64) -> u64 {
    pointer as usize as u64
}

#[cfg(all(test, feature = "mojo-routing"))]
#[test]
fn control_plane_route_self_test_passes() {
    assert!(self_test());
}
