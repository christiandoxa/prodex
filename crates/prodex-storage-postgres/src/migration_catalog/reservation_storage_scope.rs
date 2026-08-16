use crate::{PostgresMigration, PostgresMigrationPhase, PostgresMigrationVersion};

pub const RESERVATION_STORAGE_SCOPE_MIGRATION: PostgresMigration = PostgresMigration {
    version: PostgresMigrationVersion(12),
    phase: PostgresMigrationPhase::Expand,
    name: "012_reservation_storage_scope",
    sql: r#"
ALTER TABLE prodex_reservations
    ADD COLUMN IF NOT EXISTS storage_scope TEXT NOT NULL DEFAULT 'tenant-default';

DO $migration$
DECLARE
    ambiguous_reservations BIGINT;
BEGIN
    SELECT COUNT(*) INTO ambiguous_reservations
    FROM prodex_reservations reservation
    WHERE (
        SELECT COUNT(*)
        FROM prodex_budget_counters counter
        WHERE counter.tenant_id = reservation.tenant_id
          AND counter.virtual_key_id IS NOT DISTINCT FROM reservation.virtual_key_id
    ) > 1;
    IF ambiguous_reservations > 0 THEN
        RAISE EXCEPTION
            'cannot infer storage_scope for % reservation(s): multiple budget counters match',
            ambiguous_reservations;
    END IF;
END $migration$;

UPDATE prodex_reservations reservation
SET storage_scope = COALESCE(
    (
        SELECT counter.storage_scope
        FROM prodex_budget_counters counter
        WHERE counter.tenant_id = reservation.tenant_id
          AND counter.virtual_key_id IS NOT DISTINCT FROM reservation.virtual_key_id
    ),
    CASE
        WHEN reservation.virtual_key_id IS NULL THEN 'tenant-default'
        ELSE 'virtual_key:' || reservation.virtual_key_id::TEXT
    END
);

CREATE INDEX IF NOT EXISTS prodex_reservations_expired_active_idx
    ON prodex_reservations (expires_at_unix_ms, tenant_id)
    WHERE committed_at_unix_ms IS NULL AND released_at_unix_ms IS NULL;
"#,
};
