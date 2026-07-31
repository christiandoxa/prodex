use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;

pub(super) fn validate_sqlite_index_columns(
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

pub(super) fn sqlite_check_expressions(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut checks = Vec::new();
    let mut index = 0;
    while index + 5 <= bytes.len() {
        if is_sqlite_check_start(bytes, index)
            && let Some((check, end)) = sqlite_check_expression(sql, bytes, index)
        {
            checks.push(check);
            index = end;
        }
        index += 1;
    }
    checks
}

fn is_sqlite_check_start(bytes: &[u8], index: usize) -> bool {
    bytes[index..index + 5].eq_ignore_ascii_case(b"check")
        && (index == 0 || !is_sql_identifier_byte(bytes[index - 1]))
        && (index + 5 == bytes.len() || !is_sql_identifier_byte(bytes[index + 5]))
}

fn sqlite_check_expression(sql: &str, bytes: &[u8], check_index: usize) -> Option<(String, usize)> {
    let mut open = check_index + 5;
    while open < bytes.len() && bytes[open].is_ascii_whitespace() {
        open += 1;
    }
    if open >= bytes.len() || bytes[open] != b'(' {
        return None;
    }

    let start = open + 1;
    let end = sqlite_check_expression_end(bytes, start)?;
    Some((sqlite_normalize_sql(&sql[start..end]), end))
}

fn sqlite_check_expression_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut cursor = start;
    let mut quote = None;
    while cursor < bytes.len() {
        if let Some(next_cursor) = sqlite_quoted_cursor(bytes, cursor, &mut quote) {
            cursor = next_cursor;
            continue;
        }
        match bytes[cursor] {
            b'\'' | b'"' | b'`' => quote = Some(bytes[cursor]),
            b'(' => depth += 1,
            b')' if depth == 1 => return Some(cursor),
            b')' => depth -= 1,
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn sqlite_quoted_cursor(bytes: &[u8], cursor: usize, quote: &mut Option<u8>) -> Option<usize> {
    let delimiter = (*quote)?;
    if bytes[cursor] != delimiter {
        return Some(cursor + 1);
    }
    if cursor + 1 < bytes.len() && bytes[cursor + 1] == delimiter {
        return Some(cursor + 2);
    }
    *quote = None;
    Some(cursor + 1)
}

fn is_sql_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

pub(super) fn sqlite_normalize_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}
