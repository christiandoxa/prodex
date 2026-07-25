pub(crate) fn ensure_child_table<'a>(
    parent: &'a mut toml::Table,
    key: &str,
) -> &'a mut toml::Table {
    if !matches!(parent.get(key), Some(toml::Value::Table(_))) {
        parent.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    match parent.get_mut(key) {
        Some(toml::Value::Table(table)) => table,
        _ => unreachable!("child table should exist after insertion"),
    }
}
