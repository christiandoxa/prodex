use super::*;

pub(crate) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    runtime_launch::handle_gateway(args)
}
