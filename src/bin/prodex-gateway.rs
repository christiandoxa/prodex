//! Prodex data-plane gateway entrypoint.
//!
//! Serving remains gated while async HTTP/storage/provider adapters are migrated
//! behind boundary crates. The external migrator subcommand is wired now so DDL
//! has a production entrypoint outside request-serving gateway opens.

use std::path::PathBuf;
use prodex::{GatewayMigrationTarget, run_gateway_migrate};

const HELP: &str = "prodex-gateway

Data-plane gateway entrypoint.

USAGE:
    prodex-gateway --help
    prodex-gateway --version
    prodex-gateway migrate --backend sqlite --path <PATH>
    prodex-gateway migrate --backend postgres --url-env <ENV>

STATUS:
    The enterprise gateway binary is present as a dedicated composition root.
    Serving traffic is intentionally gated until the async adapter is wired to
    prodex-application, prodex-gateway-http, and prodex-gateway-core.
";

#[derive(Debug, PartialEq, Eq)]
enum MigrationTarget {
    Sqlite { path: PathBuf },
    Postgres { url_env: String },
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help") | Some("-h") => {
            print!("{HELP}");
        }
        Some("--version") | Some("-V") => {
            println!("prodex-gateway {}", env!("CARGO_PKG_VERSION"));
        }
        Some("serve") => {
            eprintln!(
                "prodex-gateway serve is not wired yet; use the legacy `prodex gateway` path until the async adapter migration is complete"
            );
            std::process::exit(2);
        }
        Some("migrate") => {
            if let Err(err) = run_migrate(parse_migrate_args(args)) {
                eprintln!("{err}");
                std::process::exit(2);
            }
        }
        Some(other) => {
            eprintln!("unknown prodex-gateway argument: {other}\n\n{HELP}");
            std::process::exit(2);
        }
    }
}

fn run_migrate(target: Result<MigrationTarget, String>) -> Result<(), String> {
    let message = match target? {
        MigrationTarget::Sqlite { path } => run_gateway_migrate(GatewayMigrationTarget::Sqlite {
            path,
        })?,
        MigrationTarget::Postgres { url_env } => {
            run_gateway_migrate(GatewayMigrationTarget::Postgres { url_env })?
        }
    };
    println!("{message}");
    Ok(())
}

fn parse_migrate_args<I>(args: I) -> Result<MigrationTarget, String>
where
    I: IntoIterator<Item = String>,
{
    let mut backend = None;
    let mut path = None;
    let mut url_env = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend" => backend = args.next(),
            "--path" => path = args.next().map(PathBuf::from),
            "--url-env" => url_env = args.next(),
            "--help" | "-h" => return Err(HELP.to_string()),
            other => return Err(format!("unknown migrate argument: {other}\n\n{HELP}")),
        }
    }
    match backend.as_deref() {
        Some("sqlite") => path
            .map(|path| MigrationTarget::Sqlite { path })
            .ok_or_else(|| "sqlite migration requires --path <PATH>".to_string()),
        Some("postgres") => url_env
            .map(|url_env| MigrationTarget::Postgres { url_env })
            .ok_or_else(|| "postgres migration requires --url-env <ENV>".to_string()),
        Some(other) => Err(format!("unsupported migration backend: {other}")),
        None => Err("migration requires --backend sqlite|postgres".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sqlite_migration_target() {
        assert_eq!(
            parse_migrate_args(
                ["--backend", "sqlite", "--path", "/tmp/prodex.sqlite"]
                    .into_iter()
                    .map(str::to_string),
            )
            .unwrap(),
            MigrationTarget::Sqlite {
                path: PathBuf::from("/tmp/prodex.sqlite")
            }
        );
    }

    #[test]
    fn parses_postgres_migration_target() {
        assert_eq!(
            parse_migrate_args(
                [
                    "--backend",
                    "postgres",
                    "--url-env",
                    "PRODEX_GATEWAY_POSTGRES_URL"
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap(),
            MigrationTarget::Postgres {
                url_env: "PRODEX_GATEWAY_POSTGRES_URL".to_string()
            }
        );
    }

    #[test]
    fn migrate_command_rejects_missing_backend() {
        assert_eq!(
            parse_migrate_args(std::iter::empty()).unwrap_err(),
            "migration requires --backend sqlite|postgres"
        );
    }
}
