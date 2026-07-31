use anyhow::{Result, bail};

pub(crate) fn ensure_child_table<'a>(
    parent: &'a mut toml::Table,
    key: &str,
) -> Result<&'a mut toml::Table> {
    if parent.contains_key(key) {
        return match parent.get_mut(key) {
            Some(toml::Value::Table(table)) => Ok(table),
            _ => bail!("configuration entry `{key}` must be a TOML table"),
        };
    }

    parent.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    match parent.get_mut(key) {
        Some(toml::Value::Table(table)) => Ok(table),
        _ => unreachable!("child table should exist after insertion"),
    }
}
