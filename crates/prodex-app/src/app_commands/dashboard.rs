use anyhow::Result;

use crate::{AppPaths, DashboardArgs, serve_dashboard};

pub(crate) fn handle_dashboard(args: DashboardArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    serve_dashboard(paths, args)
}
