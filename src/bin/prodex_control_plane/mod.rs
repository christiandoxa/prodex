//! Prodex control-plane command implementation.
//!
//! The binary is a thin composition root. Use-case details live in focused
//! modules; this module owns only CLI dispatch and process exit behavior.

mod cli;
mod http_plan;
mod publication;

use cli::{Command, Invocation};
use prodex::{DedicatedServerMode, run_enterprise_serve_or_exit};

const HELP: &str = "prodex-control-plane

Control-plane entrypoint.

USAGE:
    prodex-control-plane --help
    prodex-control-plane --version
    prodex-control-plane serve [--listen <ADDR>]
    prodex-control-plane plan-config-publication --request <path>
    prodex-control-plane plan-http-control-plane --request <path>
    prodex-control-plane deliver-config-publication --event <path> --root <path>
    prodex-control-plane publish-config-publication --event <path> --transport <path>
    prodex-control-plane compact-config-publication --transport <path> [--retain <n>]

STATUS:
    The dedicated control-plane composition root uses an async listener that
    rejects data-plane routes and dispatches through the in-process application.
";

const CONTROL_PLANE_SERVICE_NAME: &str = "prodex-control-plane";
const CONTROL_PLANE_SCOPE_NAME: &str = "prodex.control-plane";
const CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME: &str = "control_plane.http_route.plan";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_PLAN_EVENT_NAME: &str =
    "control_plane.configuration_publication.plan";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_PUBLISH_EVENT_NAME: &str =
    "control_plane.configuration_publication.publish";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_DELIVER_EVENT_NAME: &str =
    "config_publication.deliver";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_COMPACT_EVENT_NAME: &str =
    "control_plane.configuration_publication.compact";

pub(super) fn main() {
    match cli::parse_env() {
        Ok(Invocation::Help) => print!("{HELP}"),
        Ok(Invocation::Version) => {
            println!("prodex-control-plane {}", env!("CARGO_PKG_VERSION"));
        }
        Ok(Invocation::Command(command)) => dispatch(command),
        Err(error) => exit_with_error(error),
    }
}

fn dispatch(command: Command) {
    let result = match command {
        Command::Serve(args) => {
            let args = args
                .listen
                .map(|listen| vec!["--listen".to_string(), listen])
                .unwrap_or_default();
            run_enterprise_serve_or_exit(DedicatedServerMode::ControlPlane, args.into_iter(), HELP);
            return;
        }
        Command::PlanConfigPublication(args) => publication::plan(&args.request),
        Command::PlanHttpControlPlane(args) => http_plan::run(&args.request),
        Command::DeliverConfigPublication(args) => publication::deliver(&args.event, &args.root),
        Command::PublishConfigPublication(args) => {
            publication::publish(&args.event, &args.transport)
        }
        Command::CompactConfigPublication(args) => {
            publication::compact(&args.transport, args.retain)
        }
    };

    match result {
        Ok(output) => println!("{output}"),
        Err(error) => exit_with_error(error),
    }
}

fn exit_with_error(error: String) -> ! {
    eprintln!("{error}\n\n{HELP}");
    std::process::exit(2);
}
