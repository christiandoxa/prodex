use serde_json::Value;

#[derive(Debug, Clone)]
enum RuntimeMemJsonPathPart {
    Key(String),
    Index(usize),
}

pub(crate) fn runtime_mem_lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if !path.as_bytes().contains(&b'[') {
        if !path.as_bytes().contains(&b'.') {
            return value.get(path);
        }
        let mut current = value;
        for key in path.split('.') {
            if key.is_empty() {
                return None;
            }
            current = current.get(key)?;
        }
        return Some(current);
    }

    let mut current = value;
    for part in runtime_mem_json_path_parts(path)? {
        match part {
            RuntimeMemJsonPathPart::Key(key) => {
                current = current.get(key.as_str())?;
            }
            RuntimeMemJsonPathPart::Index(index) => {
                current = current.get(index)?;
            }
        }
    }
    Some(current)
}

pub(crate) fn runtime_mem_first_text_at_paths(event: &Value, paths: &[&str]) -> Option<String> {
    runtime_mem_first_text_path_at_paths(event, paths).map(|(_, text)| text)
}

pub(crate) fn runtime_mem_first_text_path_at_paths<'a>(
    event: &Value,
    paths: &'a [&'a str],
) -> Option<(&'a str, String)> {
    paths.iter().find_map(|path| {
        runtime_mem_lookup_json_path(event, path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| (*path, text.to_string()))
    })
}

pub(crate) fn runtime_mem_set_json_path(value: &mut Value, path: &str, new_value: Value) {
    let Some(parts) = runtime_mem_json_path_parts(path) else {
        return;
    };
    if parts.is_empty() {
        return;
    }
    let mut current = value;
    for (index, part) in parts.iter().enumerate() {
        let is_last = index + 1 == parts.len();
        match part {
            RuntimeMemJsonPathPart::Key(key) if is_last => {
                if let Value::Object(object) = current {
                    object.insert(key.clone(), new_value);
                }
                return;
            }
            RuntimeMemJsonPathPart::Key(key) => {
                if !current.is_object() {
                    *current = serde_json::json!({});
                }
                let object = current
                    .as_object_mut()
                    .expect("json path container should be object");
                current = object
                    .entry(key.clone())
                    .or_insert_with(|| serde_json::json!({}));
            }
            RuntimeMemJsonPathPart::Index(array_index) if is_last => {
                if let Value::Array(array) = current
                    && let Some(slot) = array.get_mut(*array_index)
                {
                    *slot = new_value;
                }
                return;
            }
            RuntimeMemJsonPathPart::Index(array_index) => {
                let Value::Array(array) = current else {
                    return;
                };
                let Some(next) = array.get_mut(*array_index) else {
                    return;
                };
                current = next;
            }
        }
    }
}

fn runtime_mem_json_path_parts(path: &str) -> Option<Vec<RuntimeMemJsonPathPart>> {
    let mut parts = Vec::new();
    for raw_part in path.split('.') {
        if raw_part.is_empty() {
            return None;
        }
        runtime_mem_push_json_path_parts(raw_part, &mut parts)?;
    }
    Some(parts)
}

fn runtime_mem_push_json_path_parts(
    raw_part: &str,
    parts: &mut Vec<RuntimeMemJsonPathPart>,
) -> Option<()> {
    let mut rest = raw_part;
    if let Some(bracket_index) = rest.find('[') {
        if bracket_index > 0 {
            parts.push(RuntimeMemJsonPathPart::Key(
                rest[..bracket_index].to_string(),
            ));
        }
        rest = &rest[bracket_index..];
    } else {
        parts.push(RuntimeMemJsonPathPart::Key(rest.to_string()));
        return Some(());
    }

    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let close_index = inner.find(']')?;
        let index = inner[..close_index].parse::<usize>().ok()?;
        parts.push(RuntimeMemJsonPathPart::Index(index));
        rest = &inner[close_index + 1..];
        if !rest.is_empty() && !rest.starts_with('[') {
            return None;
        }
    }
    Some(())
}
