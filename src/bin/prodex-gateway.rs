//! Prodex data-plane gateway entrypoint.
//!
//! The public listener uses the bounded async compatibility front while the
//! existing gateway backend remains isolated on loopback.

use prodex::{
    DedicatedServerMode, GatewayMigrationTarget, OtlpLogAttribute,
    deliver_pending_config_publication_events_to_gateway_runtime, otlp_http_log_export_status,
    run_enterprise_serve_or_exit, run_gateway_migrate,
};
use std::path::PathBuf;

const HELP: &str = "prodex-gateway

Data-plane gateway entrypoint.

USAGE:
    prodex-gateway --help
    prodex-gateway --version
    prodex-gateway migrate --backend sqlite --path <PATH>
    prodex-gateway migrate --backend postgres --url-ref <NAME> --secret-provider <PROVIDER> --secret-root <PATH> [--tls-mode verify-full|disable] [--tls-ca <PATH>]
    prodex-gateway migrate --backend postgres --url-env <ENV> [--tls-mode verify-full|disable] [--tls-ca <PATH>]
    prodex-gateway serve [--listen <ADDR>]
    prodex-gateway consume-config-publication --transport <PATH> --replica <ID> --root <PATH>

STATUS:
    The dedicated data-plane composition root uses an async, route-isolated
    listener. Compatibility execution remains on a bounded loopback backend.
";

const GATEWAY_SERVICE_NAME: &str = "prodex-gateway";
const GATEWAY_SCOPE_NAME: &str = "prodex.gateway";
const GATEWAY_MIGRATE_EVENT_NAME: &str = "gateway.migrate";
const GATEWAY_CONFIG_PUBLICATION_CONSUME_EVENT_NAME: &str = "config_publication.consume";

#[derive(Debug, PartialEq, Eq)]
enum MigrationTarget {
    Sqlite {
        path: PathBuf,
    },
    Postgres {
        url_env: String,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
    },
    PostgresProjected {
        reference: prodex_domain::SecretRef,
        projected_root: PathBuf,
        tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct ConfigPublicationConsumptionTarget {
    transport: PathBuf,
    replica: String,
    root: PathBuf,
}

fn encode_config_publication_consumption_summary(
    delivery: &prodex::ConfigPublicationTransportDeliveryPlan,
) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        GATEWAY_SERVICE_NAME,
        GATEWAY_SCOPE_NAME,
        GATEWAY_CONFIG_PUBLICATION_CONSUME_EVENT_NAME,
        {
            let mut attributes = vec![OtlpLogAttribute::u64(
                "delivered_event_count",
                delivery.delivered_event_count as u64,
            )];
            if let Some(version) = delivery.runtime_policy_version {
                attributes.push(OtlpLogAttribute::u64(
                    "runtime_policy_version",
                    version as u64,
                ));
            }
            attributes
        },
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "transport_root": delivery.transport_root,
        "replica": delivery.replica,
        "root": delivery.root,
        "delivered_event_count": delivery.delivered_event_count,
        "runtime_policy_version": delivery.runtime_policy_version,
        "otlp_log_export": otlp_log_export,
        "delivery_metrics": delivery.delivery_metrics.iter().map(|metric| {
            serde_json::json!({
                "metric_name": metric.metric_name,
                "target": metric.target_label.as_metric_label().map(|(_, value)| value).unwrap_or("invalid"),
                "result": metric.result_label.as_metric_label().map(|(_, value)| value).unwrap_or("invalid"),
                "increment": metric.increment,
            })
        }).collect::<Vec<_>>(),
    }))
    .map_err(|err| format!("failed to encode config publication consumption summary: {err}"))
}

fn encode_gateway_migration_summary(backend: &str, message: &str) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        GATEWAY_SERVICE_NAME,
        GATEWAY_SCOPE_NAME,
        GATEWAY_MIGRATE_EVENT_NAME,
        vec![
            OtlpLogAttribute::bool("migrated", true),
            OtlpLogAttribute::string("backend", backend),
        ],
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "migrated": true,
        "backend": backend,
        "message": message,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode gateway migration summary: {err}"))
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
            run_enterprise_serve_or_exit(DedicatedServerMode::DataPlane, args, HELP);
        }
        Some("migrate") => {
            if let Err(err) = run_migrate(parse_migrate_args(args)) {
                eprintln!("{err}");
                std::process::exit(2);
            }
        }
        Some("consume-config-publication") => {
            if let Err(err) = run_consume_config_publication(parse_config_publication_args(args)) {
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
    let (message, backend) = match target? {
        MigrationTarget::Sqlite { path } => (
            run_gateway_migrate(GatewayMigrationTarget::Sqlite { path })?,
            "sqlite",
        ),
        MigrationTarget::Postgres { url_env, tls } => (
            run_gateway_migrate(GatewayMigrationTarget::Postgres { url_env, tls })?,
            "postgres",
        ),
        MigrationTarget::PostgresProjected {
            reference,
            projected_root,
            tls,
        } => (
            run_gateway_migrate(GatewayMigrationTarget::PostgresProjected {
                reference,
                projected_root,
                tls,
            })?,
            "postgres",
        ),
    };
    let output = encode_gateway_migration_summary(backend, &message)?;
    println!("{output}");
    Ok(())
}

fn run_consume_config_publication(
    target: Result<ConfigPublicationConsumptionTarget, String>,
) -> Result<(), String> {
    let target = target?;
    let delivery = deliver_pending_config_publication_events_to_gateway_runtime(
        &target.transport,
        &target.replica,
        &target.root,
    )
    .map_err(|err| err.to_string())?;
    let output = encode_config_publication_consumption_summary(&delivery)?;
    println!("{output}");
    Ok(())
}

fn parse_migrate_args<I>(args: I) -> Result<MigrationTarget, String>
where
    I: IntoIterator<Item = String>,
{
    let mut backend = None;
    let mut path = None;
    let mut url_env = None;
    let mut url_ref = None;
    let mut secret_provider = None;
    let mut secret_root = None;
    let mut tls_mode = None;
    let mut tls_ca = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend" => backend = args.next(),
            "--path" => path = args.next().map(PathBuf::from),
            "--url-env" => url_env = args.next(),
            "--url-ref" => url_ref = args.next(),
            "--secret-provider" => secret_provider = args.next(),
            "--secret-root" => secret_root = args.next().map(PathBuf::from),
            "--tls-mode" => tls_mode = args.next(),
            "--tls-ca" => tls_ca = args.next().map(PathBuf::from),
            "--help" | "-h" => return Err(HELP.to_string()),
            other => return Err(format!("unknown migrate argument: {other}\n\n{HELP}")),
        }
    }
    match backend.as_deref() {
        Some("sqlite") => path
            .map(|path| MigrationTarget::Sqlite { path })
            .ok_or_else(|| "sqlite migration requires --path <PATH>".to_string()),
        Some("postgres") => {
            let tls = match tls_mode.as_deref().unwrap_or("verify-full") {
                "verify-full" => {
                    prodex_storage_postgres_runtime::PostgresTlsConfig::verify_full(tls_ca)
                }
                "disable" if tls_ca.is_some() => {
                    return Err("--tls-ca requires --tls-mode verify-full".to_string());
                }
                "disable" => prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable(),
                _ => return Err("--tls-mode must be verify-full or disable".to_string()),
            };
            match (url_env, url_ref, secret_provider, secret_root) {
                (Some(url_env), None, None, None) => Ok(MigrationTarget::Postgres { url_env, tls }),
                (None, Some(name), Some(provider), Some(projected_root)) => {
                    let reference = prodex_domain::SecretRef::new(
                        provider,
                        name,
                        None::<String>,
                    );
                    if !reference.is_well_formed() {
                        return Err("postgres migration secret reference is invalid".to_string());
                    }
                    Ok(MigrationTarget::PostgresProjected {
                        reference,
                        projected_root,
                        tls,
                    })
                }
                (None, None, None, None) => Err(
                    "postgres migration requires --url-ref/--secret-provider/--secret-root or --url-env"
                        .to_string(),
                ),
                _ => Err(
                    "postgres migration secret source flags are incomplete or conflicting"
                        .to_string(),
                ),
            }
        }
        Some(other) => Err(format!("unsupported migration backend: {other}")),
        None => Err("migration requires --backend sqlite|postgres".to_string()),
    }
}

fn parse_config_publication_args<I>(args: I) -> Result<ConfigPublicationConsumptionTarget, String>
where
    I: IntoIterator<Item = String>,
{
    let mut transport = None;
    let mut replica = None;
    let mut root = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--transport" => transport = args.next().map(PathBuf::from),
            "--replica" => replica = args.next(),
            "--root" => root = args.next().map(PathBuf::from),
            "--help" | "-h" => return Err(HELP.to_string()),
            other => {
                return Err(format!(
                    "unknown consume-config-publication argument: {other}\n\n{HELP}"
                ));
            }
        }
    }
    Ok(ConfigPublicationConsumptionTarget {
        transport: transport
            .ok_or_else(|| "consume-config-publication requires --transport <PATH>".to_string())?,
        replica: replica
            .ok_or_else(|| "consume-config-publication requires --replica <ID>".to_string())?,
        root: root
            .ok_or_else(|| "consume-config-publication requires --root <PATH>".to_string())?,
    })
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
                url_env: "PRODEX_GATEWAY_POSTGRES_URL".to_string(),
                tls: prodex_storage_postgres_runtime::PostgresTlsConfig::verify_full(None),
            }
        );
    }

    #[test]
    fn parses_projected_postgres_migration_target() {
        assert_eq!(
            parse_migrate_args(
                [
                    "--backend",
                    "postgres",
                    "--url-ref",
                    "PRODEX_GATEWAY_POSTGRES_URL",
                    "--secret-provider",
                    "kubernetes",
                    "--secret-root",
                    "/run/secrets/prodex",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap(),
            MigrationTarget::PostgresProjected {
                reference: prodex_domain::SecretRef::new(
                    "kubernetes",
                    "PRODEX_GATEWAY_POSTGRES_URL",
                    None::<String>,
                ),
                projected_root: PathBuf::from("/run/secrets/prodex"),
                tls: prodex_storage_postgres_runtime::PostgresTlsConfig::verify_full(None),
            }
        );
    }

    #[test]
    fn postgres_migration_rejects_conflicting_secret_sources() {
        assert_eq!(
            parse_migrate_args(
                [
                    "--backend",
                    "postgres",
                    "--url-env",
                    "PRODEX_GATEWAY_POSTGRES_URL",
                    "--url-ref",
                    "PRODEX_GATEWAY_POSTGRES_URL",
                    "--secret-provider",
                    "kubernetes",
                    "--secret-root",
                    "/run/secrets/prodex",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap_err(),
            "postgres migration secret source flags are incomplete or conflicting"
        );
    }

    #[test]
    fn parses_explicit_plaintext_postgres_migration_target() {
        assert_eq!(
            parse_migrate_args(
                [
                    "--backend",
                    "postgres",
                    "--url-env",
                    "PRODEX_GATEWAY_POSTGRES_URL",
                    "--tls-mode",
                    "disable",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap(),
            MigrationTarget::Postgres {
                url_env: "PRODEX_GATEWAY_POSTGRES_URL".to_string(),
                tls: prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable(),
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

    #[test]
    fn parses_config_publication_consumption_target() {
        assert_eq!(
            parse_config_publication_args(
                [
                    "--transport",
                    "/tmp/transport",
                    "--replica",
                    "gateway-a",
                    "--root",
                    "/tmp/root"
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap(),
            ConfigPublicationConsumptionTarget {
                transport: PathBuf::from("/tmp/transport"),
                replica: "gateway-a".to_string(),
                root: PathBuf::from("/tmp/root"),
            }
        );
    }
}
