use super::*;

fn prodex_path_is_under_root(root: &Path, path: &Path) -> bool {
    let root = normalize_path_for_compare(root);
    let path = normalize_path_for_compare(path);
    path == root || path.starts_with(root)
}

fn prodex_file_modified_epoch(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
}

fn prodex_history_epoch_from_json_value(value: &serde_json::Value) -> Option<i64> {
    fn normalize_epoch(value: i64) -> i64 {
        if value.abs() > 100_000_000_000 {
            value / 1_000
        } else {
            value
        }
    }

    for key in ["ts", "timestamp", "created_at", "updated_at"] {
        let Some(value) = value.get(key) else {
            continue;
        };
        if let Some(epoch) = value.as_i64() {
            return Some(normalize_epoch(epoch));
        }
        if let Some(text) = value.as_str() {
            if let Ok(epoch) = text.parse::<i64>() {
                return Some(normalize_epoch(epoch));
            }
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
                return Some(parsed.timestamp());
            }
        }
    }

    None
}

fn prodex_history_line_epoch(line: &str) -> Option<i64> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    prodex_history_epoch_from_json_value(&value)
}

fn prune_jsonl_history_file_at(
    path: &Path,
    oldest_allowed: i64,
    prune_unstamped_old_file: bool,
) -> usize {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return 0;
    }

    let file_is_old =
        prodex_file_modified_epoch(path).is_some_and(|modified| modified < oldest_allowed);
    let Ok(input) = fs::File::open(path) else {
        return 0;
    };
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return 0;
    };
    let tmp_path = path.with_file_name(format!(
        ".{file_name}.prodex-cleanup-{}.tmp",
        std::process::id()
    ));
    let _ = fs::remove_file(&tmp_path);
    let Ok(output) = fs::File::create(&tmp_path) else {
        return 0;
    };

    let mut reader = io::BufReader::new(input);
    let mut writer = io::BufWriter::new(output);
    let mut raw = String::new();
    let mut removed = 0usize;
    let mut retained = 0usize;

    loop {
        raw.clear();
        let Ok(bytes_read) = reader.read_line(&mut raw) else {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        };
        if bytes_read == 0 {
            break;
        }

        let line = raw.trim_end_matches(['\r', '\n']);
        let is_stale = prodex_history_line_epoch(line)
            .map(|epoch| epoch < oldest_allowed)
            .unwrap_or(prune_unstamped_old_file && file_is_old);
        if is_stale {
            removed += 1;
            continue;
        }

        if retained > 0 && writeln!(writer).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        if write!(writer, "{line}").is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        retained += 1;
    }

    if removed == 0 {
        let _ = fs::remove_file(&tmp_path);
        return 0;
    }
    if writer.flush().is_err() {
        let _ = fs::remove_file(&tmp_path);
        return 0;
    }

    if retained == 0 {
        if fs::remove_file(path).is_err() {
            let _ = fs::remove_file(&tmp_path);
            return 0;
        }
        let _ = fs::remove_file(&tmp_path);
        return removed;
    }

    if fs::rename(&tmp_path, path)
        .or_else(|_| {
            fs::copy(&tmp_path, path)?;
            fs::remove_file(&tmp_path)
        })
        .is_ok()
    {
        removed
    } else {
        let _ = fs::remove_file(&tmp_path);
        0
    }
}

fn prodex_chat_history_file_is_owned(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jsonl") || extension.eq_ignore_ascii_case("json")
        })
}

fn prodex_session_path_date(path: &Path) -> Option<chrono::NaiveDate> {
    let parts = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    for window in parts.windows(3) {
        let year = window[0];
        let month = window[1];
        let day = window[2];
        if year.len() != 4
            || month.len() != 2
            || day.len() != 2
            || !year.chars().all(|ch| ch.is_ascii_digit())
            || !month.chars().all(|ch| ch.is_ascii_digit())
            || !day.chars().all(|ch| ch.is_ascii_digit())
        {
            continue;
        }

        let Ok(year) = year.parse::<i32>() else {
            continue;
        };
        let Ok(month) = month.parse::<u32>() else {
            continue;
        };
        let Ok(day) = day.parse::<u32>() else {
            continue;
        };
        if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
            return Some(date);
        }
    }
    None
}

fn prodex_local_midnight_epoch(date: chrono::NaiveDate) -> Option<i64> {
    let naive = date.and_hms_opt(0, 0, 0)?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

fn prodex_session_file_is_stale(path: &Path, oldest_allowed: i64) -> bool {
    if let Some(date) = prodex_session_path_date(path) {
        return date
            .succ_opt()
            .and_then(prodex_local_midnight_epoch)
            .is_some_and(|next_day_epoch| next_day_epoch <= oldest_allowed);
    }

    prodex_file_modified_epoch(path).is_some_and(|modified| modified < oldest_allowed)
}

fn cleanup_codex_session_history_tree_at(root: &Path, oldest_allowed: i64) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            removed += cleanup_codex_session_history_tree_at(&path, oldest_allowed);
            let _ = fs::remove_dir(&path);
            continue;
        }
        if metadata.is_file()
            && prodex_chat_history_file_is_owned(&path)
            && prodex_session_file_is_stale(&path, oldest_allowed)
            && fs::remove_file(&path).is_ok()
        {
            removed += 1;
        }
    }

    removed
}

fn cleanup_claude_project_history_tree_at(root: &Path, oldest_allowed: i64) -> usize {
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            removed += cleanup_claude_project_history_tree_at(&path, oldest_allowed);
            let _ = fs::remove_dir(&path);
            continue;
        }
        if metadata.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
        {
            removed += prune_jsonl_history_file_at(&path, oldest_allowed, true);
        }
    }

    removed
}

fn cleanup_codex_chat_history_root_at(root: &Path, oldest_allowed: i64) -> usize {
    prune_jsonl_history_file_at(&root.join("history.jsonl"), oldest_allowed, false)
        + cleanup_codex_session_history_tree_at(&root.join("sessions"), oldest_allowed)
}

fn cleanup_claude_chat_history_root_at(root: &Path, oldest_allowed: i64) -> usize {
    cleanup_claude_project_history_tree_at(&root.join("projects"), oldest_allowed)
}

pub(super) fn cleanup_prodex_chat_history_at(
    paths: &AppPaths,
    state: &AppState,
    now: SystemTime,
) -> usize {
    let now_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let oldest_allowed = now_epoch.saturating_sub(PRODEX_CHAT_HISTORY_RETENTION_SECONDS);
    let mut removed = 0usize;

    let mut codex_roots = BTreeSet::new();
    for root in [&paths.shared_codex_root, &paths.legacy_shared_codex_root] {
        if prodex_path_is_under_root(&paths.root, root) {
            codex_roots.insert(root.clone());
        }
    }
    for profile in state.profiles.values() {
        if profile.managed && prodex_path_is_under_root(&paths.root, &profile.codex_home) {
            codex_roots.insert(profile.codex_home.clone());
        }
    }
    for root in codex_roots {
        removed += cleanup_codex_chat_history_root_at(&root, oldest_allowed);
        removed += cleanup_claude_chat_history_root_at(
            &runtime_proxy_claude_config_dir(&root),
            oldest_allowed,
        );
    }

    let shared_claude_root = runtime_proxy_shared_claude_config_dir(paths);
    if prodex_path_is_under_root(&paths.root, &shared_claude_root) {
        removed += cleanup_claude_chat_history_root_at(&shared_claude_root, oldest_allowed);
    }

    removed
}
