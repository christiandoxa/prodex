use super::{AppServerBrokerArgs, Result};
use anyhow::bail;
use terminal_ui::print_stdout_line;

pub(crate) fn handle_app_server_broker(args: AppServerBrokerArgs) -> Result<()> {
    if args.experimental_stdio {
        bail!(
            "app-server broker stdio mode is not implemented; default app-server remains direct passthrough"
        );
    }

    let contract = crate::app_server_broker::app_server_broker_contract_json();
    if args.json {
        print_stdout_line(&serde_json::to_string_pretty(&contract)?);
    } else {
        print_stdout_line(
            "app-server broker: skeleton only; direct Codex app-server passthrough remains default",
        );
    }
    Ok(())
}
