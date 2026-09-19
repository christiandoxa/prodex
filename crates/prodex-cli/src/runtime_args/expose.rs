use super::SuperArgs;
use clap::Args;

#[derive(Args, Clone, Debug)]
pub struct SuperExposeArgs {
    /// Loopback address for the local MCP endpoint.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:0")]
    pub listen: String,
    /// Keep the endpoint local. This is the default and remains accepted for 0.429.x compatibility.
    #[arg(long)]
    pub no_tunnel: bool,
    /// Public tunnel compatibility flag. The lean 0.430 expose surface is local-only.
    #[arg(long, conflicts_with = "no_tunnel")]
    pub tunnel: bool,
    /// Optional display name for this workspace endpoint.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    #[command(flatten)]
    pub super_args: SuperArgs,
}
