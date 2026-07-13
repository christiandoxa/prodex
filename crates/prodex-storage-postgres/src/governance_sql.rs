//! Driver-free PostgreSQL SQL port for governance lifecycle execution.

use prodex_storage::GovernanceArtifactKind;

use crate::PostgresStatement;

pub const INSERT_GOVERNANCE_REVISION_ARTIFACT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "insert_governance_revision_artifact",
    sql: r#"
INSERT INTO prodex_governance_revision_artifacts (
    tenant_id, artifact_kind, revision_id, artifact_checksum,
    compiled_artifact, created_by, created_at_unix_ms
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (tenant_id, artifact_kind, revision_id) DO NOTHING
RETURNING revision_id
"#,
};

pub const LOAD_GOVERNANCE_REVISION_ARTIFACT_STATEMENT: PostgresStatement = PostgresStatement {
    name: "load_governance_revision_artifact",
    sql: r#"
SELECT artifact_checksum, compiled_artifact, created_by, created_at_unix_ms
FROM prodex_governance_revision_artifacts
WHERE tenant_id = $1 AND artifact_kind = $2 AND revision_id = $3
"#,
};

pub const APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT: PostgresStatement = PostgresStatement {
    name: "append_audit_outbox_atomic",
    sql: r#"
WITH audit_insert AS (
    INSERT INTO prodex_audit_log (
        tenant_id, audit_event_id, previous_digest, event_digest,
        occurred_at_unix_ms, principal_id, action, resource_kind,
        resource_id, outcome, reason_code
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
    RETURNING tenant_id, audit_event_id, occurred_at_unix_ms
)
INSERT INTO prodex_siem_outbox (
    tenant_id, event_id, audit_event_id, event_envelope,
    attempt_count, next_attempt_at_unix_ms, created_at_unix_ms,
    delivered_at_unix_ms
)
SELECT tenant_id, $12, audit_event_id, $13, 0,
       occurred_at_unix_ms, occurred_at_unix_ms, NULL
FROM audit_insert
RETURNING event_id
"#,
};

pub const LOAD_DUE_SIEM_OUTBOX_STATEMENT: PostgresStatement = PostgresStatement {
    name: "load_due_siem_outbox",
    sql: r#"
SELECT tenant_id, event_id, audit_event_id, event_envelope, attempt_count
FROM prodex_siem_outbox
WHERE delivered_at_unix_ms IS NULL AND next_attempt_at_unix_ms <= $1
ORDER BY next_attempt_at_unix_ms, event_id
FOR UPDATE SKIP LOCKED
LIMIT $2
"#,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresGovernancePointerStatements {
    pub load: PostgresStatement,
    pub compare_and_swap: PostgresStatement,
}

pub fn postgres_governance_pointer_statements(
    kind: GovernanceArtifactKind,
) -> PostgresGovernancePointerStatements {
    match kind {
        GovernanceArtifactKind::Policy => PostgresGovernancePointerStatements {
            load: LOAD_POLICY_POINTER_STATEMENT,
            compare_and_swap: CAS_POLICY_POINTER_STATEMENT,
        },
        GovernanceArtifactKind::ClassificationRules => PostgresGovernancePointerStatements {
            load: LOAD_CLASSIFICATION_POINTER_STATEMENT,
            compare_and_swap: CAS_CLASSIFICATION_POINTER_STATEMENT,
        },
        GovernanceArtifactKind::ProviderRegistry => PostgresGovernancePointerStatements {
            load: LOAD_PROVIDER_REGISTRY_POINTER_STATEMENT,
            compare_and_swap: CAS_PROVIDER_REGISTRY_POINTER_STATEMENT,
        },
        GovernanceArtifactKind::RoutingScores => PostgresGovernancePointerStatements {
            load: LOAD_ROUTING_SCORE_POINTER_STATEMENT,
            compare_and_swap: CAS_ROUTING_SCORE_POINTER_STATEMENT,
        },
    }
}

macro_rules! pointer_statements {
    ($load:ident, $cas:ident, $table:literal) => {
        const $load: PostgresStatement = PostgresStatement {
            name: stringify!($load),
            sql: concat!(
                "SELECT active_revision_id, last_known_good_revision_id, etag ",
                "FROM ",
                $table,
                " WHERE tenant_id = $1 FOR UPDATE"
            ),
        };
        const $cas: PostgresStatement = PostgresStatement {
            name: stringify!($cas),
            sql: concat!(
                "INSERT INTO ",
                $table,
                " (tenant_id, active_revision_id, last_known_good_revision_id, etag, updated_at_unix_ms) ",
                "SELECT $1, $2, $3, $4, $5 WHERE $6::text IS NULL ",
                "ON CONFLICT (tenant_id) DO UPDATE SET ",
                "active_revision_id = EXCLUDED.active_revision_id, ",
                "last_known_good_revision_id = EXCLUDED.last_known_good_revision_id, ",
                "etag = EXCLUDED.etag, updated_at_unix_ms = EXCLUDED.updated_at_unix_ms ",
                "WHERE ",
                $table,
                ".etag = $6 RETURNING etag"
            ),
        };
    };
}

pointer_statements!(
    LOAD_POLICY_POINTER_STATEMENT,
    CAS_POLICY_POINTER_STATEMENT,
    "prodex_policy_pointers"
);
pointer_statements!(
    LOAD_CLASSIFICATION_POINTER_STATEMENT,
    CAS_CLASSIFICATION_POINTER_STATEMENT,
    "prodex_classification_rule_pointers"
);
pointer_statements!(
    LOAD_PROVIDER_REGISTRY_POINTER_STATEMENT,
    CAS_PROVIDER_REGISTRY_POINTER_STATEMENT,
    "prodex_provider_registry_pointers"
);
pointer_statements!(
    LOAD_ROUTING_SCORE_POINTER_STATEMENT,
    CAS_ROUTING_SCORE_POINTER_STATEMENT,
    "prodex_routing_score_pointers"
);
