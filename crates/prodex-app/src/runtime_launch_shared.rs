use crate::{AppPaths, RuntimeProxyEndpoint};

pub(super) type RuntimeLaunchRequest<'a> = prodex_runtime_launch::RuntimeLaunchRequest<'a>;
pub(super) type PreparedRuntimeLaunch =
    prodex_runtime_launch::PreparedRuntimeLaunch<AppPaths, RuntimeProxyEndpoint>;
