use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use zeroize::Zeroizing;

use super::{
    ConfigPublicationDeliveryMetricPlan, ConfigPublicationEventPlan,
    ConfigPublicationTransportEventFile, RuntimePolicyPublicationDeliveryPlan,
    config_publication_transport_delivery_metrics, config_publication_transport_event_file,
    deliver_config_publication_event_to_gateway_runtime,
    validate_config_publication_transport_replica,
};

const BATCH_LIMIT: i64 = 256;
const REPLICA_LEASE_MS: i64 = 5 * 60 * 1_000;

pub struct ConfigPublicationPostgresTransport {
    pub(super) database_url: Zeroizing<String>,
    pub(super) tls: prodex_storage_postgres_runtime::PostgresTlsConfig,
}

impl fmt::Debug for ConfigPublicationPostgresTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigPublicationPostgresTransport")
            .field("database_url", &"<redacted>")
            .field("tls", &self.tls)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationPostgresPublishPlan {
    pub event_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationPostgresDeliveryPlan {
    pub replica: String,
    pub root: PathBuf,
    pub delivered_event_count: usize,
    pub runtime_policy_version: Option<u32>,
    pub delivery_metrics: [ConfigPublicationDeliveryMetricPlan; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPublicationPostgresCompactionPlan {
    pub replica_count: usize,
    pub eligible_event_count: usize,
    pub removed_event_count: usize,
    pub retained_event_count: usize,
}

pub fn runtime_config_publication_postgres_transport() -> Result<ConfigPublicationPostgresTransport>
{
    let paths = prodex_core::AppPaths::discover()?;
    let policy = prodex_runtime_policy::load_runtime_policy_cached(&paths.root)?
        .context("Postgres config publication requires policy.toml")?;
    let (database_url, tls) =
        crate::app_commands::runtime_launch::gateway_config::resolve_config_publication_postgres_transport(
            &paths,
            &policy.gateway,
            &policy.secrets,
        )?;
    Ok(ConfigPublicationPostgresTransport {
        database_url: Zeroizing::new(database_url),
        tls,
    })
}

pub fn publish_config_publication_event_to_postgres_transport(
    event: &ConfigPublicationEventPlan,
    transport: &ConfigPublicationPostgresTransport,
) -> Result<ConfigPublicationPostgresPublishPlan> {
    let record = config_publication_transport_event_file(event);
    let payload = serde_json::to_string(&record)?;
    let published_at_unix_ms = postgres_i64(record.published_at_unix_ms)?;
    let mut client = connect(transport)?;
    client.execute(
        "INSERT INTO prodex_config_publication_events \
         (event_id, published_at_unix_ms, event_payload) \
         VALUES ($1, $2, $3::text::jsonb) ON CONFLICT (event_id) DO NOTHING",
        &[&record.event_id, &published_at_unix_ms, &payload],
    )?;
    Ok(ConfigPublicationPostgresPublishPlan {
        event_id: record.event_id,
    })
}

pub fn deliver_pending_postgres_config_publication_events_to_gateway_runtime(
    transport: &ConfigPublicationPostgresTransport,
    replica: &str,
    root: &Path,
) -> Result<ConfigPublicationPostgresDeliveryPlan> {
    deliver_pending_postgres_config_publication_events_with_activation(
        transport,
        replica,
        root,
        |_| Ok(()),
    )
}

pub fn deliver_pending_postgres_config_publication_events_with_activation(
    transport: &ConfigPublicationPostgresTransport,
    replica: &str,
    root: &Path,
    mut activate: impl FnMut(&RuntimePolicyPublicationDeliveryPlan) -> Result<()>,
) -> Result<ConfigPublicationPostgresDeliveryPlan> {
    validate_config_publication_transport_replica(replica)?;
    let mut client = connect(transport)?;
    client.execute(
        "INSERT INTO prodex_config_publication_replicas \
         (replica_id, registered_at_unix_ms, last_seen_at_unix_ms) \
         SELECT $1, observed.now_ms, observed.now_ms FROM ( \
             SELECT (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT AS now_ms \
         ) observed ON CONFLICT (replica_id) DO UPDATE SET \
         last_seen_at_unix_ms = GREATEST( \
             prodex_config_publication_replicas.last_seen_at_unix_ms, \
             EXCLUDED.last_seen_at_unix_ms \
         )",
        &[&replica],
    )?;
    let lock_key = format!("prodex-config-publication:{replica}");
    let locked: bool = client
        .query_one(
            "SELECT pg_try_advisory_lock(hashtextextended($1, 0))",
            &[&lock_key],
        )?
        .get(0);
    if !locked {
        bail!("config publication replica is already being consumed");
    }
    let result = deliver_locked(&mut client, replica, root, &mut activate);
    let unlocked: Result<bool> = client
        .query_one(
            "SELECT pg_advisory_unlock(hashtextextended($1, 0))",
            &[&lock_key],
        )
        .map(|row| row.get(0))
        .map_err(Into::into);
    match (result, unlocked) {
        (Err(error), _) => Err(error),
        (Ok(_), Ok(false)) => bail!("config publication replica lock was lost"),
        (Ok(plan), Ok(true)) => Ok(plan),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn deliver_locked(
    client: &mut postgres::Client,
    replica: &str,
    root: &Path,
    activate: &mut impl FnMut(&RuntimePolicyPublicationDeliveryPlan) -> Result<()>,
) -> Result<ConfigPublicationPostgresDeliveryPlan> {
    let rows = client.query(
        "SELECT event.event_payload::text \
         FROM prodex_config_publication_events event \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM prodex_config_publication_acks ack \
             WHERE ack.replica_id = $1 AND ack.event_id = event.event_id \
         ) \
         ORDER BY event.published_at_unix_ms, event.event_id \
         LIMIT $2",
        &[&replica, &BATCH_LIMIT],
    )?;
    let mut delivered_event_count = 0usize;
    let mut runtime_policy_version = None;
    for row in rows {
        let payload: String = row.get(0);
        let record: ConfigPublicationTransportEventFile = serde_json::from_str(&payload)
            .context("failed to parse Postgres config publication event")?;
        let delivery =
            deliver_config_publication_event_to_gateway_runtime(&record.to_event_plan()?, root)?;
        activate(&delivery)?;
        client.execute(
            "INSERT INTO prodex_config_publication_acks \
             (replica_id, event_id, delivered_at_unix_ms) \
             VALUES ($1, $2, (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT) \
             ON CONFLICT (replica_id, event_id) DO NOTHING",
            &[&replica, &record.event_id],
        )?;
        touch_replica(client, replica)?;
        runtime_policy_version = delivery.runtime_policy_version;
        delivered_event_count += 1;
    }
    touch_replica(client, replica)?;
    Ok(ConfigPublicationPostgresDeliveryPlan {
        replica: replica.to_string(),
        root: root.to_path_buf(),
        delivered_event_count,
        runtime_policy_version,
        delivery_metrics: config_publication_transport_delivery_metrics(
            delivered_event_count as u64,
        )?,
    })
}

fn touch_replica(client: &mut postgres::Client, replica: &str) -> Result<()> {
    client.execute(
        "UPDATE prodex_config_publication_replicas SET last_seen_at_unix_ms = GREATEST( \
             last_seen_at_unix_ms, \
             (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT \
         ) WHERE replica_id = $1",
        &[&replica],
    )?;
    Ok(())
}

pub fn compact_postgres_config_publication_transport(
    transport: &ConfigPublicationPostgresTransport,
    retain: usize,
) -> Result<ConfigPublicationPostgresCompactionPlan> {
    let mut client = connect(transport)?;
    let mut transaction = client.transaction()?;
    transaction.batch_execute(
        "LOCK TABLE prodex_config_publication_replicas IN SHARE ROW EXCLUSIVE MODE",
    )?;
    transaction.execute(
        "DELETE FROM prodex_config_publication_replicas \
         WHERE last_seen_at_unix_ms < \
             (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT - $1",
        &[&REPLICA_LEASE_MS],
    )?;
    let replica_count = postgres_usize(
        transaction
            .query_one(
                "SELECT COUNT(*) FROM prodex_config_publication_replicas",
                &[],
            )?
            .get::<_, i64>(0),
    )?;
    let retained_event_count_before = postgres_usize(
        transaction
            .query_one("SELECT COUNT(*) FROM prodex_config_publication_events", &[])?
            .get::<_, i64>(0),
    )?;
    if replica_count == 0 {
        transaction.commit()?;
        return Ok(ConfigPublicationPostgresCompactionPlan {
            replica_count,
            eligible_event_count: 0,
            removed_event_count: 0,
            retained_event_count: retained_event_count_before,
        });
    }
    let eligible_predicate = "NOT EXISTS ( \
        SELECT 1 FROM prodex_config_publication_replicas replica \
        WHERE NOT EXISTS ( \
            SELECT 1 FROM prodex_config_publication_acks ack \
            WHERE ack.replica_id = replica.replica_id \
              AND ack.event_id = event.event_id \
        ) \
    )";
    let eligible_event_count = postgres_usize(
        transaction
            .query_one(
                &format!(
                    "SELECT COUNT(*) FROM prodex_config_publication_events event WHERE {eligible_predicate}"
                ),
                &[],
            )?
            .get::<_, i64>(0),
    )?;
    // Keep one acknowledged event so a newly registered or lease-expired
    // replica can always trigger a reload of the current local policy.
    let retain =
        i64::try_from(retain.max(1)).context("config publication retention is too large")?;
    let removed_event_count = usize::try_from(transaction.execute(
        &format!(
            "DELETE FROM prodex_config_publication_events \
             WHERE event_id IN ( \
                 SELECT event.event_id FROM prodex_config_publication_events event \
                 WHERE {eligible_predicate} \
                 ORDER BY event.published_at_unix_ms DESC, event.event_id DESC \
                 OFFSET $1 \
             )"
        ),
        &[&retain],
    )?)
    .context("config publication removal count is invalid")?;
    transaction.commit()?;
    Ok(ConfigPublicationPostgresCompactionPlan {
        replica_count,
        eligible_event_count,
        removed_event_count,
        retained_event_count: retained_event_count_before.saturating_sub(removed_event_count),
    })
}

fn connect(transport: &ConfigPublicationPostgresTransport) -> Result<postgres::Client> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(
        transport.database_url.as_str(),
        &transport.tls,
    )
    .context("failed to connect to Postgres config publication transport")?;
    let observed = client
        .query_one(
            "SELECT MAX(version) FROM prodex_enterprise_schema_migrations",
            &[],
        )
        .context("Postgres config publication schema has not been migrated")?
        .get::<_, Option<i64>>(0)
        .context("Postgres config publication schema has no version")?;
    let observed =
        u32::try_from(observed).context("Postgres config publication schema version is invalid")?;
    prodex_storage_postgres::plan_postgres_backend_open(
        prodex_storage_postgres::PostgresBackendOpenMode::GatewayStartup,
        Some(prodex_storage_postgres::PostgresMigrationVersion(observed)),
    )
    .map_err(|error| anyhow!(error))?;
    Ok(client)
}

fn postgres_i64(value: u64) -> Result<i64> {
    i64::try_from(value).context("config publication timestamp is too large")
}

fn postgres_usize(value: i64) -> Result<usize> {
    usize::try_from(value).context("config publication count is invalid")
}
