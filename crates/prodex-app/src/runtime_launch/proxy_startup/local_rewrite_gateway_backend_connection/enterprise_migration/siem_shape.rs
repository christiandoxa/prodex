use super::super::runtime_gateway_postgres_table_exists;
use anyhow::{Result, anyhow, bail};
use postgres::Client as PostgresClient;
use rusqlite::{Connection, OptionalExtension};

pub(super) fn sqlite_siem_outbox_has_v13_marker(conn: &Connection) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM pragma_table_xinfo('prodex_siem_outbox')
            WHERE name IN ('claim_token', 'claim_expires_at_unix_ms')
        ) OR EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE name IN (
                'prodex_siem_outbox_due_claim_idx',
                'prodex_siem_outbox_claim_pair_insert',
                'prodex_siem_outbox_claim_pair_update'
            )
        )",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

pub(super) fn validate_sqlite_siem_outbox_shape(
    conn: &Connection,
    require_v13: bool,
) -> Result<()> {
    let table_sql = conn
        .query_row(
            "SELECT sql FROM sqlite_master
             WHERE type = 'table' AND name = 'prodex_siem_outbox'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("SIEM outbox table is missing"))?;

    let expected_columns = [
        ("tenant_id", "TEXT", 1, None, 1),
        ("event_id", "TEXT", 1, None, 2),
        ("audit_event_id", "TEXT", 1, None, 0),
        ("event_envelope", "TEXT", 1, None, 0),
        ("attempt_count", "INTEGER", 1, Some("0"), 0),
        ("next_attempt_at_unix_ms", "INTEGER", 1, None, 0),
        ("created_at_unix_ms", "INTEGER", 1, None, 0),
        ("delivered_at_unix_ms", "INTEGER", 0, None, 0),
        ("claim_token", "TEXT", 0, None, 0),
        ("claim_expires_at_unix_ms", "INTEGER", 0, None, 0),
    ];
    let columns = conn
        .prepare(
            "SELECT name, type, \"notnull\", dflt_value, pk, hidden
             FROM pragma_table_xinfo('prodex_siem_outbox') ORDER BY cid",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let expected_columns = if require_v13 {
        &expected_columns[..]
    } else {
        &expected_columns[..8]
    };
    if columns.len() != expected_columns.len()
        || columns.iter().zip(expected_columns).any(
            |((name, sql_type, notnull, default, primary_key, hidden), expected)| {
                name != expected.0
                    || !sql_type.eq_ignore_ascii_case(expected.1)
                    || *notnull != expected.2
                    || default.as_deref().map(sqlite_normalize_sql)
                        != expected.3.map(sqlite_normalize_sql)
                    || *primary_key != expected.4
                    || *hidden != 0
            },
        )
    {
        bail!(
            "cannot infer gateway sqlite enterprise schema: SIEM outbox columns are not the known shape"
        );
    }

    let mut checks = sqlite_check_expressions(&table_sql);
    checks.sort();
    let mut expected_checks = vec![sqlite_normalize_sql("attempt_count >= 0")];
    if require_v13 {
        expected_checks.extend([
            sqlite_normalize_sql("claim_token IS NULL OR length(claim_token) BETWEEN 1 AND 128"),
            sqlite_normalize_sql(
                "claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms >= 0",
            ),
        ]);
    }
    expected_checks.sort();
    if checks != expected_checks {
        bail!(
            "cannot infer gateway sqlite enterprise schema: SIEM outbox checks are not the known shape"
        );
    }

    let foreign_keys = conn
        .prepare(
            "SELECT \"table\", \"from\", \"to\", on_update, on_delete, \"match\"
             FROM pragma_foreign_key_list('prodex_siem_outbox') ORDER BY id, seq",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if foreign_keys
        != [(
            "prodex_tenants".to_string(),
            "tenant_id".to_string(),
            "tenant_id".to_string(),
            "NO ACTION".to_string(),
            "NO ACTION".to_string(),
            "NONE".to_string(),
        )]
    {
        bail!(
            "cannot infer gateway sqlite enterprise schema: SIEM outbox foreign keys are not the known shape"
        );
    }

    let indexes = conn
        .prepare(
            "SELECT name, \"unique\", origin, partial
             FROM pragma_index_list('prodex_siem_outbox') ORDER BY name",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut primary_index = false;
    let mut unique_index = false;
    let mut due_claim_index = false;
    let primary_columns = ["tenant_id", "event_id"];
    let unique_columns = ["tenant_id", "audit_event_id"];
    let due_claim_columns = [
        "delivered_at_unix_ms",
        "next_attempt_at_unix_ms",
        "event_id",
        "claim_expires_at_unix_ms",
    ];
    for (name, unique, origin, partial) in indexes {
        let expected = if origin == "pk" {
            if primary_index || unique != 1 || partial != 0 {
                bail!(
                    "cannot infer gateway sqlite enterprise schema: SIEM outbox primary index is not the known shape"
                );
            }
            primary_index = true;
            primary_columns.as_slice()
        } else if origin == "u" {
            if unique_index || unique != 1 || partial != 0 {
                bail!(
                    "cannot infer gateway sqlite enterprise schema: SIEM outbox unique index is not the known shape"
                );
            }
            unique_index = true;
            unique_columns.as_slice()
        } else if name == "prodex_siem_outbox_due_claim_idx" {
            if !require_v13 || due_claim_index || origin != "c" || unique != 0 || partial != 0 {
                bail!(
                    "cannot infer gateway sqlite enterprise schema: SIEM outbox lease index is not the known shape"
                );
            }
            due_claim_index = true;
            due_claim_columns.as_slice()
        } else {
            bail!(
                "cannot infer gateway sqlite enterprise schema: SIEM outbox has an unknown index"
            );
        };
        validate_sqlite_index_columns(conn, &name, expected)?;
    }
    if !primary_index || !unique_index || due_claim_index != require_v13 {
        bail!("cannot infer gateway sqlite enterprise schema: SIEM outbox indexes are incomplete");
    }

    let marker_objects = conn
        .prepare(
            "SELECT name, type, tbl_name, sql FROM sqlite_master
             WHERE name IN (
                 'prodex_siem_outbox_due_claim_idx',
                 'prodex_siem_outbox_claim_pair_insert',
                 'prodex_siem_outbox_claim_pair_update'
             ) ORDER BY name",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (name, object_type, owner, sql) in marker_objects {
        let expected_type = if name == "prodex_siem_outbox_due_claim_idx" {
            "index"
        } else {
            "trigger"
        };
        if object_type != expected_type || owner != "prodex_siem_outbox" {
            bail!(
                "cannot infer gateway sqlite enterprise schema: SIEM outbox marker has the wrong owner or object type"
            );
        }
        if expected_type == "trigger" {
            let expected_sql = if name == "prodex_siem_outbox_claim_pair_insert" {
                "CREATE TRIGGER prodex_siem_outbox_claim_pair_insert
                 BEFORE INSERT ON prodex_siem_outbox
                 WHEN (NEW.claim_token IS NULL) <> (NEW.claim_expires_at_unix_ms IS NULL)
                 BEGIN
                     SELECT RAISE(ABORT, 'SIEM outbox claim fields must be paired');
                 END"
            } else {
                "CREATE TRIGGER prodex_siem_outbox_claim_pair_update
                 BEFORE UPDATE OF claim_token, claim_expires_at_unix_ms ON prodex_siem_outbox
                 WHEN (NEW.claim_token IS NULL) <> (NEW.claim_expires_at_unix_ms IS NULL)
                 BEGIN
                     SELECT RAISE(ABORT, 'SIEM outbox claim fields must be paired');
                 END"
            };
            if sqlite_normalize_sql(sql.as_deref().unwrap_or_default())
                != sqlite_normalize_sql(expected_sql)
            {
                bail!(
                    "cannot infer gateway sqlite enterprise schema: SIEM outbox trigger behavior is not the known shape"
                );
            }
        }
    }
    let trigger_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'trigger' AND tbl_name = 'prodex_siem_outbox'",
        [],
        |row| row.get(0),
    )?;
    if trigger_count != if require_v13 { 2 } else { 0 } {
        bail!("cannot infer gateway sqlite enterprise schema: SIEM outbox has unknown triggers");
    }
    if require_v13 {
        let trigger_names: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'trigger' AND tbl_name = 'prodex_siem_outbox' ORDER BY name",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if trigger_names
            != [
                "prodex_siem_outbox_claim_pair_insert".to_string(),
                "prodex_siem_outbox_claim_pair_update".to_string(),
            ]
        {
            bail!(
                "cannot infer gateway sqlite enterprise schema: SIEM outbox triggers are not the known shape"
            );
        }
    }
    Ok(())
}

fn validate_sqlite_index_columns(
    conn: &Connection,
    index_name: &str,
    expected: &[&str],
) -> Result<()> {
    let index_name = index_name.replace('\'', "''");
    let query = format!(
        "SELECT seqno, name, \"desc\", coll, key
         FROM pragma_index_xinfo('{index_name}') ORDER BY seqno"
    );
    let rows = conn
        .prepare(&query)?
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut key_columns = Vec::new();
    for (seqno, name, descending, collation, key) in rows {
        if key == 0 {
            continue;
        }
        if seqno < 0 || descending != 0 || !collation.eq_ignore_ascii_case("BINARY") {
            bail!(
                "cannot infer gateway sqlite enterprise schema: SIEM outbox index metadata is not the known shape"
            );
        }
        key_columns.push(name.ok_or_else(|| {
            anyhow!("cannot infer gateway sqlite enterprise schema: SIEM outbox index has an expression key")
        })?);
    }
    if key_columns != expected {
        bail!(
            "cannot infer gateway sqlite enterprise schema: SIEM outbox index columns are not the known order"
        );
    }
    Ok(())
}

fn sqlite_check_expressions(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut checks = Vec::new();
    let mut index = 0;
    while index + 5 <= bytes.len() {
        if bytes[index..index + 5].eq_ignore_ascii_case(b"check")
            && (index == 0 || !is_sql_identifier_byte(bytes[index - 1]))
            && (index + 5 == bytes.len() || !is_sql_identifier_byte(bytes[index + 5]))
        {
            let mut open = index + 5;
            while open < bytes.len() && bytes[open].is_ascii_whitespace() {
                open += 1;
            }
            if open < bytes.len() && bytes[open] == b'(' {
                let mut depth = 1;
                let start = open + 1;
                let mut cursor = start;
                let mut quote = None;
                while cursor < bytes.len() {
                    let byte = bytes[cursor];
                    if let Some(delimiter) = quote {
                        if byte == delimiter {
                            if cursor + 1 < bytes.len() && bytes[cursor + 1] == delimiter {
                                cursor += 2;
                                continue;
                            }
                            quote = None;
                        }
                    } else if matches!(byte, b'\'' | b'"' | b'`') {
                        quote = Some(byte);
                    } else if byte == b'(' {
                        depth += 1;
                    } else if byte == b')' {
                        depth -= 1;
                        if depth == 0 {
                            checks.push(sqlite_normalize_sql(&sql[start..cursor]));
                            index = cursor;
                            break;
                        }
                    }
                    cursor += 1;
                }
            }
        }
        index += 1;
    }
    checks
}

fn is_sql_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn sqlite_normalize_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Default)]
pub(super) struct PostgresSiemOutboxShape {
    pub(super) complete_v6: bool,
}

pub(super) fn validate_postgres_siem_outbox_shape(
    client: &mut PostgresClient,
) -> Result<PostgresSiemOutboxShape> {
    if !runtime_gateway_postgres_table_exists(client, "prodex_siem_outbox")? {
        return Ok(PostgresSiemOutboxShape::default());
    }
    let due_index_has_wrong_owner: bool = client
        .query_one(
            "SELECT EXISTS(
                SELECT 1
                FROM pg_catalog.pg_class relation_row
                JOIN pg_catalog.pg_namespace namespace_row
                  ON namespace_row.oid = relation_row.relnamespace
                WHERE namespace_row.nspname = current_schema()
                  AND relation_row.relname = 'prodex_siem_outbox_due_claim_idx'
                  AND NOT EXISTS (
                      SELECT 1
                      FROM pg_catalog.pg_index index_row
                      JOIN pg_catalog.pg_class table_row
                        ON table_row.oid = index_row.indrelid
                      JOIN pg_catalog.pg_namespace table_namespace
                        ON table_namespace.oid = table_row.relnamespace
                      WHERE index_row.indexrelid = relation_row.oid
                        AND table_namespace.nspname = current_schema()
                        AND table_row.relname = 'prodex_siem_outbox'
                  )
            )",
            &[],
        )?
        .get(0);
    if due_index_has_wrong_owner {
        bail!(
            "cannot infer gateway postgres enterprise schema: SIEM outbox lease index has the wrong owner"
        );
    }

    let columns = client.query(
        "SELECT attribute.attname,
                pg_catalog.format_type(attribute.atttypid, attribute.atttypmod),
                attribute.attnotnull,
                COALESCE(pg_catalog.pg_get_expr(default_value.adbin, default_value.adrelid), ''),
                attribute.attgenerated::TEXT,
                attribute.attidentity::TEXT
         FROM pg_catalog.pg_attribute attribute
         JOIN pg_catalog.pg_class table_row ON table_row.oid = attribute.attrelid
         JOIN pg_catalog.pg_namespace namespace_row ON namespace_row.oid = table_row.relnamespace
         LEFT JOIN pg_catalog.pg_attrdef default_value
           ON default_value.adrelid = attribute.attrelid
          AND default_value.adnum = attribute.attnum
         WHERE namespace_row.nspname = current_schema()
           AND table_row.relname = 'prodex_siem_outbox'
           AND attribute.attnum > 0
           AND NOT attribute.attisdropped
         ORDER BY attribute.attnum",
        &[],
    )?;
    let expected_columns = [
        ("tenant_id", "uuid", true, ""),
        ("event_id", "uuid", true, ""),
        ("audit_event_id", "uuid", true, ""),
        ("event_envelope", "jsonb", true, ""),
        ("attempt_count", "integer", true, "0"),
        ("next_attempt_at_unix_ms", "bigint", true, ""),
        ("created_at_unix_ms", "bigint", true, ""),
        ("delivered_at_unix_ms", "bigint", false, ""),
        ("claim_token", "uuid", false, ""),
        ("claim_expires_at_unix_ms", "bigint", false, ""),
    ];
    let mut has_claim_token = false;
    let mut has_claim_expires_at = false;
    if columns.len() < 8
        || columns.len() > expected_columns.len()
        || columns.iter().any(|row| {
            let name: &str = row.get(0);
            !expected_columns.iter().any(|expected| expected.0 == name)
        })
    {
        bail!("cannot infer gateway postgres enterprise schema: SIEM outbox has unknown columns");
    }
    for (row, expected) in columns.iter().zip(expected_columns) {
        let name: &str = row.get(0);
        let sql_type: &str = row.get(1);
        let not_null: bool = row.get(2);
        let default_value: &str = row.get(3);
        let generated: &str = row.get(4);
        let identity: &str = row.get(5);
        if name != expected.0
            || !sql_type.eq_ignore_ascii_case(expected.1)
            || not_null != expected.2
            || postgres_normalize_sql(default_value) != expected.3
            || !generated.is_empty()
            || !identity.is_empty()
        {
            bail!(
                "cannot infer gateway postgres enterprise schema: SIEM outbox columns are not the known shape"
            );
        }
        has_claim_token |= name == "claim_token";
        has_claim_expires_at |= name == "claim_expires_at_unix_ms";
    }

    let indexes = client.query(
        "SELECT index_class.relname,
                index_row.indisprimary,
                index_row.indisunique,
                index_row.indpred IS NULL,
                index_row.indnkeyatts::INT,
                index_row.indnatts::INT,
                EXISTS (
                    SELECT 1 FROM pg_catalog.pg_constraint constraint_row
                    WHERE constraint_row.conindid = index_row.indexrelid
                      AND constraint_row.contype IN ('p', 'u')
                ),
                COALESCE((
                    SELECT string_agg(attribute.attname, ',' ORDER BY key.ord)
                    FROM unnest(index_row.indkey) WITH ORDINALITY AS key(attnum, ord)
                    LEFT JOIN pg_catalog.pg_attribute attribute
                      ON attribute.attrelid = index_row.indrelid
                     AND attribute.attnum = key.attnum
                    WHERE key.ord <= index_row.indnkeyatts
                ), ''),
                COALESCE((
                    SELECT count(*) = count(*) FILTER (WHERE option = 0)
                    FROM unnest(index_row.indoption) AS options(option)
                ), true)
         FROM pg_catalog.pg_index index_row
         JOIN pg_catalog.pg_class index_class ON index_class.oid = index_row.indexrelid
         JOIN pg_catalog.pg_namespace index_namespace ON index_namespace.oid = index_class.relnamespace
         JOIN pg_catalog.pg_class table_row ON table_row.oid = index_row.indrelid
         JOIN pg_catalog.pg_namespace table_namespace ON table_namespace.oid = table_row.relnamespace
         WHERE table_namespace.nspname = current_schema()
           AND table_row.relname = 'prodex_siem_outbox'
           AND index_namespace.nspname = current_schema()
         ORDER BY index_class.relname",
        &[],
    )?;
    let mut primary_index = false;
    let mut unique_index = false;
    let mut due_claim_index = false;
    for row in indexes {
        let name: &str = row.get(0);
        let primary: bool = row.get(1);
        let unique: bool = row.get(2);
        let no_predicate: bool = row.get(3);
        let key_count: i32 = row.get(4);
        let column_count: i32 = row.get(5);
        let constraint_index: bool = row.get(6);
        let columns: &str = row.get(7);
        let ascending: bool = row.get(8);
        if name == "prodex_siem_outbox_due_claim_idx" {
            if due_claim_index
                || primary
                || unique
                || !no_predicate
                || key_count != 5
                || column_count != 5
                || constraint_index
                || columns
                    != "tenant_id,delivered_at_unix_ms,next_attempt_at_unix_ms,claim_expires_at_unix_ms,event_id"
                || !ascending
            {
                bail!(
                    "cannot infer gateway postgres enterprise schema: SIEM outbox lease index is not the known shape"
                );
            }
            due_claim_index = true;
        } else if primary {
            if primary_index
                || !unique
                || !no_predicate
                || key_count != 2
                || column_count != 2
                || !constraint_index
                || columns != "tenant_id,event_id"
                || !ascending
            {
                bail!(
                    "cannot infer gateway postgres enterprise schema: SIEM outbox primary index is not the known shape"
                );
            }
            primary_index = true;
        } else if unique {
            if unique_index
                || !no_predicate
                || key_count != 2
                || column_count != 2
                || !constraint_index
                || columns != "tenant_id,audit_event_id"
                || !ascending
            {
                bail!(
                    "cannot infer gateway postgres enterprise schema: SIEM outbox unique index is not the known shape"
                );
            }
            unique_index = true;
        } else {
            bail!(
                "cannot infer gateway postgres enterprise schema: SIEM outbox has an unknown index"
            );
        }
    }
    if !primary_index || !unique_index {
        bail!(
            "cannot infer gateway postgres enterprise schema: SIEM outbox indexes are incomplete"
        );
    }

    let constraints = client.query(
        "SELECT constraint_row.contype::TEXT,
                constraint_row.condeferrable,
                pg_catalog.pg_get_constraintdef(constraint_row.oid)
         FROM pg_catalog.pg_constraint constraint_row
         JOIN pg_catalog.pg_class table_row ON table_row.oid = constraint_row.conrelid
         JOIN pg_catalog.pg_namespace namespace_row ON namespace_row.oid = table_row.relnamespace
         WHERE namespace_row.nspname = current_schema()
           AND table_row.relname = 'prodex_siem_outbox'
         ORDER BY constraint_row.oid",
        &[],
    )?;
    let mut primary_constraint = false;
    let mut unique_constraint = false;
    let mut tenant_foreign_key = false;
    let mut audit_foreign_key = false;
    let mut attempt_check = false;
    let mut bounded_check = false;
    let mut claim_pair_check = false;
    for row in constraints {
        let constraint_type: &str = row.get(0);
        let deferrable: bool = row.get(1);
        let definition: &str = row.get(2);
        if deferrable {
            bail!(
                "cannot infer gateway postgres enterprise schema: SIEM outbox has an unknown constraint"
            );
        }
        let definition = postgres_normalize_sql(definition);
        match constraint_type {
            "p" if definition == "primarykeytenant_id,event_id" => {
                if primary_constraint {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate primary constraints"
                    );
                }
                primary_constraint = true;
            }
            "u" if definition == "uniquetenant_id,audit_event_id" => {
                if unique_constraint {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate unique constraints"
                    );
                }
                unique_constraint = true;
            }
            "f" if definition == "foreignkeytenant_idreferencesprodex_tenantstenant_id" => {
                if tenant_foreign_key {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate tenant constraints"
                    );
                }
                tenant_foreign_key = true;
            }
            "f" if definition
                == "foreignkeytenant_id,audit_event_idreferencesprodex_audit_logtenant_id,audit_event_id" =>
            {
                if audit_foreign_key {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate audit constraints"
                    );
                }
                audit_foreign_key = true;
            }
            "c" if definition == "checkattempt_count>=0" => {
                if attempt_check {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate attempt constraints"
                    );
                }
                attempt_check = true;
            }
            "c" if definition
                == "checkoctet_lengthevent_envelope::text<=1048576andnext_attempt_at_unix_ms>=0andcreated_at_unix_ms>=0anddelivered_at_unix_msisnullordelivered_at_unix_ms>=created_at_unix_ms" =>
            {
                if bounded_check {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate bound constraints"
                    );
                }
                bounded_check = true;
            }
            "c" if definition
                == "checkclaim_tokenisnullandclaim_expires_at_unix_msisnullorclaim_tokenisnotnullandclaim_expires_at_unix_msisnotnull" =>
            {
                if claim_pair_check {
                    bail!(
                        "cannot infer gateway postgres enterprise schema: SIEM outbox has duplicate claim constraints"
                    );
                }
                claim_pair_check = true;
            }
            _ => bail!(
                "cannot infer gateway postgres enterprise schema: SIEM outbox has an unknown constraint"
            ),
        }
    }
    if !primary_constraint || !unique_constraint || !tenant_foreign_key || !attempt_check {
        bail!(
            "cannot infer gateway postgres enterprise schema: SIEM outbox constraints are incomplete"
        );
    }
    if audit_foreign_key != bounded_check {
        bail!(
            "cannot infer gateway postgres enterprise schema: SIEM outbox hardening constraints are incomplete"
        );
    }
    let complete_v6 =
        has_claim_token || has_claim_expires_at || due_claim_index || claim_pair_check;
    if complete_v6
        && (!has_claim_token || !has_claim_expires_at || !due_claim_index || !claim_pair_check)
    {
        bail!(
            "cannot infer gateway postgres enterprise schema: SIEM outbox leasing shape is incomplete or unrecognized"
        );
    }
    Ok(PostgresSiemOutboxShape { complete_v6 })
}

fn postgres_normalize_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace() && *character != '(' && *character != ')')
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .replace("notvalid", "")
}
