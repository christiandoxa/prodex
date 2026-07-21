use prodex_storage_postgres::{
    CONFIG_PUBLICATION_TRANSPORT_MIGRATION, REQUIRED_POSTGRES_SCHEMA_VERSION,
};

#[test]
fn config_publication_migration_is_durable_bounded_and_replica_scoped() {
    assert_eq!(
        CONFIG_PUBLICATION_TRANSPORT_MIGRATION.version,
        REQUIRED_POSTGRES_SCHEMA_VERSION
    );
    for table in [
        "prodex_config_publication_events",
        "prodex_config_publication_replicas",
        "prodex_config_publication_acks",
    ] {
        assert!(CONFIG_PUBLICATION_TRANSPORT_MIGRATION.sql.contains(table));
    }
    assert!(
        CONFIG_PUBLICATION_TRANSPORT_MIGRATION
            .sql
            .contains("octet_length(event_payload::text) <= 65536")
    );
    assert!(
        CONFIG_PUBLICATION_TRANSPORT_MIGRATION
            .sql
            .contains("PRIMARY KEY (replica_id, event_id)")
    );
}
