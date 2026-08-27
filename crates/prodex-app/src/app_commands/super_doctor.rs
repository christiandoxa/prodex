use super::capability::{collect_super_tool_statuses, print_capability_panel};
use anyhow::{Context, Result, bail};
use prodex_cli::SuperDoctorArgs;
use prodex_core::AppPaths;
use terminal_ui::print_stdout_line;

pub(crate) fn handle_super_doctor(args: SuperDoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let statuses = collect_super_tool_statuses(&paths, args.presidio);
    let ready = statuses.iter().all(|status| status.ready);

    if args.json {
        let value = serde_json::json!({
            "ready": ready,
            "strict": args.strict,
            "presidio_checked": args.presidio,
            "tools": statuses,
        });
        print_stdout_line(
            &serde_json::to_string_pretty(&value)
                .context("failed to serialize Super doctor report")?,
        )?;
    } else {
        let fields = statuses
            .iter()
            .map(|status| {
                (
                    status.name.to_string(),
                    format!("{}; {}; {}", status.status, status.check, status.detail),
                )
            })
            .collect::<Vec<_>>();
        print_capability_panel("Super Doctor", &fields)?;
    }

    if args.strict && !ready {
        bail!("Super optimizer doctor found unavailable tools");
    }
    Ok(())
}
