use clap::{Args, Parser, Subcommand};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Invocation {
    Help,
    Version,
    Command(Command),
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub(super) enum Command {
    Serve(ServeArgs),
    PlanConfigPublication(RequestArgs),
    PlanHttpControlPlane(RequestArgs),
    DeliverConfigPublication(DeliverArgs),
    PublishConfigPublication(PublishArgs),
    CompactConfigPublication(CompactArgs),
}

#[derive(Args, Debug, PartialEq, Eq)]
pub(super) struct ServeArgs {
    #[arg(long, value_name = "ADDR")]
    pub listen: Option<String>,
    #[arg(long, value_name = "PATH", requires = "config_publication_replica")]
    pub config_publication_transport: Option<PathBuf>,
    #[arg(long, value_name = "ID", requires = "config_publication_transport")]
    pub config_publication_replica: Option<String>,
}

#[derive(Args, Debug, PartialEq, Eq)]
pub(super) struct RequestArgs {
    #[arg(long, value_name = "path")]
    pub request: PathBuf,
}

#[derive(Args, Debug, PartialEq, Eq)]
pub(super) struct DeliverArgs {
    #[arg(long, value_name = "path")]
    pub event: PathBuf,
    #[arg(long, value_name = "path")]
    pub root: PathBuf,
}

#[derive(Args, Debug, PartialEq, Eq)]
pub(super) struct PublishArgs {
    #[arg(long, value_name = "path")]
    pub event: PathBuf,
    #[arg(long, value_name = "path")]
    pub transport: PathBuf,
}

#[derive(Args, Debug, PartialEq, Eq)]
pub(super) struct CompactArgs {
    #[arg(long, value_name = "path")]
    pub transport: PathBuf,
    #[arg(long, value_name = "n", default_value_t = 0)]
    pub retain: usize,
}

#[derive(Debug, Parser)]
#[command(
    name = "prodex-control-plane",
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

pub(super) fn parse_env() -> Result<Invocation, String> {
    parse_from(std::env::args_os().skip(1))
}

fn parse_from<I, T>(args: I) -> Result<Invocation, String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    match args.first().and_then(|arg| arg.to_str()) {
        None | Some("--help") | Some("-h") => return Ok(Invocation::Help),
        Some("--version") | Some("-V") => return Ok(Invocation::Version),
        _ => {}
    }

    Cli::try_parse_from(std::iter::once(OsString::from("prodex-control-plane")).chain(args))
        .map(|cli| Invocation::Command(cli.command))
        .map_err(|error| error.to_string().trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_help_and_version_invocations_preserve_legacy_modes() {
        assert_eq!(parse_from(std::iter::empty::<&str>()), Ok(Invocation::Help));
        assert_eq!(parse_from(["-h"]), Ok(Invocation::Help));
        assert_eq!(parse_from(["--version"]), Ok(Invocation::Version));
    }

    #[test]
    fn typed_parser_covers_publication_paths_and_defaults() {
        assert_eq!(
            parse_from(["plan-config-publication", "--request", "request.json"]),
            Ok(Invocation::Command(Command::PlanConfigPublication(
                RequestArgs {
                    request: PathBuf::from("request.json")
                }
            )))
        );
        assert_eq!(
            parse_from([
                "deliver-config-publication",
                "--event",
                "event.json",
                "--root",
                "runtime"
            ]),
            Ok(Invocation::Command(Command::DeliverConfigPublication(
                DeliverArgs {
                    event: PathBuf::from("event.json"),
                    root: PathBuf::from("runtime")
                }
            )))
        );
        assert_eq!(
            parse_from(["compact-config-publication", "--transport", "transport"]),
            Ok(Invocation::Command(Command::CompactConfigPublication(
                CompactArgs {
                    transport: PathBuf::from("transport"),
                    retain: 0
                }
            )))
        );
    }

    #[test]
    fn typed_parser_covers_remaining_commands() {
        assert_eq!(
            parse_from(["serve", "--listen", "127.0.0.1:8080"]),
            Ok(Invocation::Command(Command::Serve(ServeArgs {
                listen: Some("127.0.0.1:8080".to_string()),
                config_publication_transport: None,
                config_publication_replica: None,
            })))
        );
        assert_eq!(
            parse_from(["plan-http-control-plane", "--request", "http.json"]),
            Ok(Invocation::Command(Command::PlanHttpControlPlane(
                RequestArgs {
                    request: PathBuf::from("http.json")
                }
            )))
        );
        assert_eq!(
            parse_from([
                "publish-config-publication",
                "--event",
                "event.json",
                "--transport",
                "transport"
            ]),
            Ok(Invocation::Command(Command::PublishConfigPublication(
                PublishArgs {
                    event: PathBuf::from("event.json"),
                    transport: PathBuf::from("transport")
                }
            )))
        );
        assert_eq!(
            parse_from([
                "compact-config-publication",
                "--transport",
                "transport",
                "--retain",
                "7"
            ]),
            Ok(Invocation::Command(Command::CompactConfigPublication(
                CompactArgs {
                    transport: PathBuf::from("transport"),
                    retain: 7
                }
            )))
        );
    }

    #[test]
    fn typed_parser_rejects_missing_required_paths_and_invalid_retain() {
        assert!(parse_from(["plan-http-control-plane"]).is_err());
        assert!(
            parse_from([
                "compact-config-publication",
                "--transport",
                "transport",
                "--retain",
                "many"
            ])
            .is_err()
        );
    }
}
