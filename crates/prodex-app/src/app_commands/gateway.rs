use super::{GatewayArgs, Result, runtime_launch};

pub(crate) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    runtime_launch::handle_gateway(args)
}
