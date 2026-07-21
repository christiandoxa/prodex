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
    prodex-control-plane serve [--listen <ADDR>] [(--config-publication-transport <PATH>|--config-publication-postgres) [--config-publication-replica <ID>]]
    prodex-control-plane plan-config-publication --request <path>
    prodex-control-plane plan-http-control-plane --request <path>
    prodex-control-plane deliver-config-publication --event <path> --root <path>
    prodex-control-plane publish-config-publication --event <path> (--transport <path>|--postgres)
    prodex-control-plane compact-config-publication (--transport <path>|--postgres) [--retain <n>]

STATUS:
    The dedicated control-plane composition root uses an async listener that
    rejects data-plane routes and dispatches through the in-process application.
    PRODEX_CONFIG_PUBLICATION_REPLICA_ID supplies the replica ID when the
    corresponding serve argument is omitted.
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
            let mut serve_args = Vec::new();
            if let Some(listen) = args.listen {
                serve_args.extend(["--listen".to_string(), listen]);
            }
            if let Some(transport) = args.config_publication_transport {
                serve_args.extend([
                    "--config-publication-transport".to_string(),
                    transport.to_string_lossy().into_owned(),
                ]);
            }
            if args.config_publication_postgres {
                serve_args.push("--config-publication-postgres".to_string());
            }
            if let Some(replica) = args.config_publication_replica {
                serve_args.extend(["--config-publication-replica".to_string(), replica]);
            }
            run_enterprise_serve_or_exit(
                DedicatedServerMode::ControlPlane,
                serve_args.into_iter(),
                HELP,
            );
            return;
        }
        Command::PlanConfigPublication(args) => publication::plan(&args.request),
        Command::PlanHttpControlPlane(args) => http_plan::run(&args.request),
        Command::DeliverConfigPublication(args) => publication::deliver(&args.event, &args.root),
        Command::PublishConfigPublication(args) => {
            publication::publish(&args.event, args.transport.as_deref(), args.postgres)
        }
        Command::CompactConfigPublication(args) => {
            publication::compact(args.transport.as_deref(), args.postgres, args.retain)
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
