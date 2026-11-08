use super::*;
use std::fs::File;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const ALIAS: &str = "prodex-openai-governed-http";

fn canonical_history(raw: &str) -> String {
    raw.replacen(
        &format!("\"{ALIAS}\""),
        &format!("{:<width$}", "\"openai\"", width = ALIAS.len() + 2),
        1,
    )
}
const ID: &str = "01900000-0000-7000-8000-000000000501";
const OTHER_ID: &str = "01900000-0000-7000-8000-000000000502";

fn rollout(root: &Path, storage: &str, id: &str, provider: &str) -> (PathBuf, String) {
    let path = root.join(storage).join(format!("rollout-{id}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = format!(
        "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"/home/test-user/project\",\"note\":\"é 😀\",\"model_provider\" : \"{provider}\"}}}}\r\n{{\"type\":\"event_msg\",\"payload\":{{\"message\":\"{ALIAS} preserved history\"}}}}\n"
    );
    fs::write(&path, &raw).unwrap();
    (path, raw)
}

fn index(root: &Path, status: &str) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(root.join("state_5.sqlite")).unwrap();
    connection.execute_batch(
        "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL, model_provider TEXT NOT NULL);
         CREATE INDEX idx_threads_provider ON threads(model_provider);
         CREATE TABLE backfill_state (id INTEGER PRIMARY KEY, status TEXT NOT NULL);",
    ).unwrap();
    connection
        .execute("INSERT INTO backfill_state VALUES (1, ?1)", [status])
        .unwrap();
    connection
}

fn insert(connection: &rusqlite::Connection, id: &str, path: &Path, provider: &str) {
    connection
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, ?3)",
            rusqlite::params![id, path.to_str().unwrap(), provider],
        )
        .unwrap();
}

fn indexed_provider(connection: &rusqlite::Connection, id: &str) -> String {
    connection
        .query_row(
            "SELECT model_provider FROM threads WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
}

fn migrate(root: &Path, explicit: Option<&Path>) -> ProviderIdentityMigration {
    migrate_legacy_openai_provider(root, explicit, || {
        let file = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".test-maintenance.lock"))?;
        file.try_lock()?;
        Ok(Some(file))
    })
    .unwrap()
}

#[test]
fn provider_migration_changes_only_reserved_metadata_and_updates_native_index() {
    let root = test_temp_dir("provider-migration-native-index");
    let (path, raw) = rollout(&root, "sessions/2026/01/01", ID, ALIAS);
    let raw = format!("\r\n \t{raw}");
    fs::write(&path, &raw).unwrap();
    let connection = index(&root, "complete");
    insert(&connection, ID, &path, ALIAS);
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), canonical_history(&raw));
    assert_eq!(
        fs::read(path.with_extension("jsonl.prodex-provider-identity-bak")).unwrap(),
        raw.as_bytes()
    );
    assert_eq!(indexed_provider(&connection, ID), "openai");
    assert_eq!(migrate(&root, None), ProviderIdentityMigration::default());
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_keeps_native_and_custom_provider_identities() {
    let root = test_temp_dir("provider-migration-custom-identities");
    for (offset, provider) in [
        "openai",
        "prodex-openai-compatible",
        "other-provider",
        "prodex-openai-governed-http-custom",
        "PRODEX-OPENAI-GOVERNED-HTTP",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("01900000-0000-7000-8000-{:012}", 600 + offset);
        let (path, raw) = rollout(&root, "sessions", &id, provider);
        let report = migrate_legacy_openai_provider(&root, Some(&path), || {
            panic!("unrelated provider must not take maintenance lock")
        })
        .unwrap();
        assert_eq!(report, ProviderIdentityMigration::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), raw);
        assert!(
            !path
                .with_extension("jsonl.prodex-provider-identity-bak")
                .exists()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_discovers_unindexed_active_sessions_but_archives_require_target() {
    let root = test_temp_dir("provider-migration-unindexed");
    let (active, raw) = rollout(&root, "sessions/2026/01/01", ID, ALIAS);
    let (archived, archived_raw) = rollout(&root, "archived_sessions", OTHER_ID, ALIAS);
    assert_eq!(migrate(&root, None).repaired, 1);
    assert_ne!(fs::read_to_string(active).unwrap(), raw);
    assert_eq!(fs::read_to_string(&archived).unwrap(), archived_raw);
    assert_eq!(migrate(&root, Some(&archived)).repaired, 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_complete_backfill_uses_index_and_explicit_uuid() {
    let root = test_temp_dir("provider-migration-complete-backfill");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let connection = index(&root, "complete");
    assert_eq!(migrate(&root, None), ProviderIdentityMigration::default());
    assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    assert_eq!(migrate(&root, Some(&path)).repaired, 1);
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_recovers_file_index_interruption_and_matches_uuid_and_path() {
    let root = test_temp_dir("provider-migration-index-recovery");
    let (path, raw) = rollout(&root, "sessions", ID, "openai");
    let (other, other_raw) = rollout(&root, "sessions", OTHER_ID, "prodex-openai-compatible");
    let connection = index(&root, "complete");
    insert(&connection, ID, &path, ALIAS);
    insert(&connection, OTHER_ID, &other, ALIAS);
    assert_eq!(
        migrate(&root, None),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 1
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    assert!(
        !path
            .with_extension("jsonl.prodex-provider-identity-bak")
            .exists()
    );
    assert_eq!(indexed_provider(&connection, ID), "openai");
    assert_eq!(fs::read_to_string(&other).unwrap(), other_raw);
    assert_eq!(indexed_provider(&connection, OTHER_ID), ALIAS);
    connection
        .execute(
            "UPDATE threads SET model_provider = ?1, rollout_path = ?2 WHERE id = ?3",
            rusqlite::params![ALIAS, other.to_str().unwrap(), ID],
        )
        .unwrap();
    assert_eq!(migrate(&root, None).repaired, 0);
    assert_eq!(indexed_provider(&connection, ID), ALIAS);
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_recovers_promoted_overlay_index_without_history_scan() {
    let root = test_temp_dir("provider-migration-overlay-index");
    let (path, _) = rollout(&root, "sessions/2026/01/01", ID, ALIAS);
    let connection = index(&root, "complete");
    let stale = root
        .join(".prodex-overlay-old")
        .join(path.strip_prefix(&root).unwrap());
    insert(&connection, ID, &stale, ALIAS);
    assert_eq!(migrate(&root, None).repaired, 1);
    assert_eq!(indexed_provider(&connection, ID), "openai");
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_prepares_all_indexed_targets_for_the_native_picker() {
    let root = test_temp_dir("provider-migration-batch");
    let (explicit, _) = rollout(&root, "archived_sessions", OTHER_ID, ALIAS);
    let connection = index(&root, "complete");
    for n in 0..40 {
        let id = format!("01900000-0000-7000-8000-{:012}", 700 + n);
        let (path, _) = rollout(&root, "sessions", &id, ALIAS);
        insert(&connection, &id, &path, ALIAS);
    }
    let report = migrate(&root, Some(&explicit));
    assert_eq!(
        report,
        ProviderIdentityMigration {
            repaired: 41,
            deferred: 0
        }
    );
    assert!(
        fs::read_to_string(explicit)
            .unwrap()
            .contains("\"model_provider\" : \"openai\"")
    );
    let remaining: i64 = connection
        .query_row(
            "SELECT count(*) FROM threads WHERE model_provider = ?1",
            [ALIAS],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_compressed_backup_preserves_original_bytes() {
    let root = test_temp_dir("provider-migration-compressed");
    let (plain, raw) = rollout(&root, "sessions", ID, ALIAS);
    let path = plain.with_extension("jsonl.zst");
    let compressed = zstd::stream::encode_all(raw.as_bytes(), 1).unwrap();
    fs::write(&path, &compressed).unwrap();
    fs::remove_file(plain).unwrap();
    assert_eq!(migrate(&root, Some(&path)).repaired, 1);
    assert_eq!(
        fs::read(path.with_extension("zst.prodex-provider-identity-bak")).unwrap(),
        compressed
    );
    let decoded = zstd::stream::decode_all(fs::read(path).unwrap().as_slice()).unwrap();
    assert_eq!(decoded, canonical_history(&raw).as_bytes());
    fs::remove_dir_all(root).unwrap();
}

struct ChildLock(Child);
impl Drop for ChildLock {
    fn drop(&mut self) {
        self.0.stdin.take();
        let _ = self.0.wait();
    }
}

fn child_lock(path: &Path) -> ChildLock {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::provider_identity::hold_provider_migration_lock",
            "--nocapture",
        ])
        .env("PRODEX_TEST_PROVIDER_LOCK", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let guard = ChildLock(child);
    let started = Instant::now();
    while !path.with_extension("ready").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "child lock not ready"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    guard
}

#[test]
fn hold_provider_migration_lock() {
    let Some(path) = std::env::var_os("PRODEX_TEST_PROVIDER_LOCK").map(PathBuf::from) else {
        return;
    };
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.lock().unwrap();
    fs::write(path.with_extension("ready"), b"ready").unwrap();
    let mut end = [0_u8; 1];
    let _ = std::io::Read::read(&mut std::io::stdin(), &mut end);
}

#[test]
fn provider_migration_defers_native_writers_and_busy_repair_without_waiting() {
    for name in [
        format!("thread-writer-locks/{ID}.lock"),
        "thread-writer-locks/.coordination.lock".into(),
        ".tmp/rollout-maintenance.lock".into(),
        ".prodex-session-repair.lock".into(),
    ] {
        let root = test_temp_dir("provider-migration-locks");
        let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
        let guard = child_lock(&root.join(name));
        let started = Instant::now();
        assert_eq!(
            migrate(&root, Some(&path)),
            ProviderIdentityMigration {
                repaired: 0,
                deferred: 1
            }
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(fs::read_to_string(&path).unwrap(), raw);
        assert!(
            !path
                .with_extension("jsonl.prodex-provider-identity-bak")
                .exists()
        );
        drop(guard);
        assert_eq!(migrate(&root, Some(&path)).repaired, 1);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn provider_migration_defers_contended_maintenance_and_streams_oversized_rollouts() {
    let root = test_temp_dir("provider-migration-deferred");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    assert_eq!(
        migrate_legacy_openai_provider(&root, Some(&path), || Ok(None)).unwrap(),
        ProviderIdentityMigration {
            repaired: 0,
            deferred: 1
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    let oversized = format!("{raw}{}", " ".repeat(SESSION_STORE_FILE_MAX_BYTES as usize));
    fs::write(&path, &oversized).unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        canonical_history(&oversized)
    );
    assert_eq!(
        fs::read_to_string(path.with_extension("jsonl.prodex-provider-identity-bak")).unwrap(),
        oversized
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn provider_migration_rejects_symlink_sources_databases_and_native_lock_namespaces() {
    use std::os::unix::fs::symlink;
    let root = test_temp_dir("provider-migration-symlinks");
    let outside = test_temp_dir("provider-migration-outside");
    let (outside_path, outside_raw) = rollout(&outside, "sessions", ID, ALIAS);
    fs::create_dir_all(root.join("sessions")).unwrap();
    let linked = root.join("sessions/linked.jsonl");
    symlink(&outside_path, &linked).unwrap();
    let outside_db = index(&outside, "complete");
    insert(&outside_db, ID, &outside_path, ALIAS);
    symlink(outside.join("state_5.sqlite"), root.join("state_5.sqlite")).unwrap();
    assert_eq!(
        migrate(&root, Some(&linked)),
        ProviderIdentityMigration::default()
    );
    assert_eq!(fs::read_to_string(&outside_path).unwrap(), outside_raw);
    assert_eq!(indexed_provider(&outside_db, ID), ALIAS);
    let (path, raw) = rollout(&root, "sessions", OTHER_ID, ALIAS);
    symlink(&outside, root.join("thread-writer-locks")).unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 0,
            deferred: 1
        }
    );
    assert_eq!(fs::read_to_string(path).unwrap(), raw);
    assert!(!outside.join(".coordination.lock").exists());
    drop(outside_db);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn provider_migration_preserves_previous_general_backup_and_requires_exact_provider_backup() {
    let root = test_temp_dir("provider-migration-backup-history");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let general = path.with_extension("jsonl.prodex-repair-bak");
    let dedicated = path.with_extension("jsonl.prodex-provider-identity-bak");
    fs::write(&general, b"older general repair history").unwrap();
    fs::write(&dedicated, b"different provider snapshot").unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 0,
            deferred: 1
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    assert_eq!(
        fs::read(&dedicated).unwrap(),
        b"different provider snapshot"
    );
    fs::remove_file(&dedicated).unwrap();
    assert_eq!(migrate(&root, Some(&path)).repaired, 1);
    assert_eq!(fs::read(general).unwrap(), b"older general repair history");
    assert_eq!(fs::read(&dedicated).unwrap(), raw.as_bytes());
    // A prior interrupted attempt with the same exact snapshot is safe to retry.
    fs::write(&path, &raw).unwrap();
    assert_eq!(migrate(&root, Some(&path)).repaired, 1);
    assert_eq!(fs::read(dedicated).unwrap(), raw.as_bytes());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_recovers_after_busy_sqlite_writer() {
    let root = test_temp_dir("provider-migration-busy-sqlite");
    let (path, _) = rollout(&root, "sessions", ID, ALIAS);
    let connection = index(&root, "complete");
    insert(&connection, ID, &path, ALIAS);
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = Instant::now();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 0,
            deferred: 1
        }
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("\"model_provider\" : \"openai\"")
    );
    assert_eq!(indexed_provider(&connection, ID), ALIAS);
    connection.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        migrate(&root, None),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(indexed_provider(&connection, ID), "openai");
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_streams_large_history_without_changing_transcript() {
    let root = test_temp_dir("provider-migration-streamed-history");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let raw = format!(
        "{raw}{}\n",
        " ".repeat(SESSION_STORE_FILE_MAX_BYTES as usize * 3)
    );
    fs::write(&path, &raw).unwrap();
    let connection = index(&root, "complete");
    insert(&connection, ID, &path, ALIAS);
    let before_time = fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), canonical_history(&raw));
    assert_eq!(
        fs::read_to_string(path.with_extension("jsonl.prodex-provider-identity-bak")).unwrap(),
        raw
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), raw.len() as u64);
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        before_time
    );
    assert_eq!(indexed_provider(&connection, ID), "openai");
    drop(connection);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn streamed_provider_repair_preserves_concurrent_append_and_cleans_temporary_files() {
    let root = test_temp_dir("provider-migration-streamed-append");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let transaction = repair_transaction::SessionHeaderRepair::try_begin(&root, &path, 1024 * 1024)
        .unwrap()
        .unwrap();
    let prefix = transaction
        .prefix()
        .replacen(&format!("\"{ALIAS}\""), "\"openai\"", 1);
    let appended = b"{\"type\":\"event_msg\",\"payload\":{\"message\":\"concurrent writer\"}}\n";
    let result = transaction.commit_after_prepare(prefix.as_bytes(), || {
        File::options()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(appended)
            .unwrap();
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(&path).unwrap(),
        [raw.as_bytes(), appended].concat()
    );
    let names = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        !names.iter().any(
            |name| name.contains("prodex-repair-tmp") || name.contains("provider-identity-bak")
        )
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provider_migration_streams_large_compressed_history_and_retains_encoded_backup() {
    let root = test_temp_dir("provider-migration-large-compressed");
    let (plain, raw) = rollout(&root, "sessions", ID, ALIAS);
    let raw = format!(
        "{raw}{}\n",
        " ".repeat(SESSION_STORE_FILE_MAX_BYTES as usize * 3)
    );
    let path = plain.with_extension("jsonl.zst");
    let encoded = zstd::stream::encode_all(raw.as_bytes(), 1).unwrap();
    fs::write(&path, &encoded).unwrap();
    fs::remove_file(plain).unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(
        fs::read(path.with_extension("zst.prodex-provider-identity-bak")).unwrap(),
        encoded
    );
    assert_eq!(
        zstd::stream::decode_all(File::open(&path).unwrap()).unwrap(),
        canonical_history(&raw).as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_staging_copy_does_not_poison_provider_backup_retry() {
    let root = test_temp_dir("provider-backup-staging-retry");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let leftover =
        path.with_extension("jsonl.prodex-provider-identity-bak.prodex-repair-tmp-interrupted");
    fs::write(&leftover, &raw.as_bytes()[..raw.len() / 2]).unwrap();
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(
        fs::read(path.with_extension("jsonl.prodex-provider-identity-bak")).unwrap(),
        raw.as_bytes()
    );
    // Unknown staging owners are not deleted by a launch-time history repair.
    assert_eq!(
        fs::read(&leftover).unwrap(),
        raw.as_bytes()[..raw.len() / 2]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hold_streamed_provider_publication() {
    let Some(root) = std::env::var_os("PRODEX_TEST_HEADER_REPAIR_ROOT").map(PathBuf::from) else {
        return;
    };
    let path = root.join("sessions").join(format!("rollout-{ID}.jsonl"));
    let transaction = repair_transaction::SessionHeaderRepair::try_begin(&root, &path, 1024 * 1024)
        .unwrap()
        .unwrap();
    let prefix = canonical_history(transaction.prefix());
    transaction
        .commit_after_prepare(prefix.as_bytes(), || {
            fs::write(root.join("staged.ready"), b"ready").unwrap();
            let mut release = [0_u8];
            let _ = std::io::Read::read_exact(&mut std::io::stdin(), &mut release);
        })
        .unwrap();
}

#[test]
fn provider_migration_recovers_after_process_termination_before_publication() {
    let root = test_temp_dir("provider-migration-terminated-publication");
    let (path, raw) = rollout(&root, "sessions", ID, ALIAS);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "tests::provider_identity::hold_streamed_provider_publication",
            "--nocapture",
        ])
        .env("PRODEX_TEST_HEADER_REPAIR_ROOT", &root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.join("staged.ready").exists() && Instant::now() < deadline {
        assert!(
            child.try_wait().unwrap().is_none(),
            "publication helper exited early"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = root.join("staged.ready").exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(ready, "publication helper did not reach durable staging");
    assert_eq!(fs::read(&path).unwrap(), raw.as_bytes());
    assert_eq!(
        fs::read(path.with_extension("jsonl.prodex-provider-identity-bak")).unwrap(),
        raw.as_bytes()
    );
    assert_eq!(
        migrate(&root, Some(&path)),
        ProviderIdentityMigration {
            repaired: 1,
            deferred: 0
        }
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), canonical_history(&raw));
    fs::remove_dir_all(root).unwrap();
}
