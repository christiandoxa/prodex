//! Resolve parent-owned native threads from rollout headers, never conversation bodies.
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{
    collect_exact_session_paths_for_selector, full_codex_session_id,
    report::session_value_metadata, visit_session_lines,
};

const MAX_PARENT_DEPTH: usize = 32;

/// A verified persisted conversation root (or directly resumable legacy thread).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResumeTarget {
    pub id: String,
    pub path: PathBuf,
}

/// Resolve a launch notification/footer to its conversation root without mutating history.
/// Missing input sessions return `None`; broken, ambiguous, or cyclic ancestry is an error.
pub fn resolve_session_root(home: &Path, id: &str) -> Result<Option<SessionResumeTarget>> {
    resolve_root(home, id, RootPurpose::LaunchIdentity)
}

/// Resolve V2 children to their owning conversation before native TUI startup.
/// V1 children and independent forks retain their existing direct-resume semantics.
pub fn resolve_native_resume_target(home: &Path, id: &str) -> Result<Option<SessionResumeTarget>> {
    resolve_root(home, id, RootPurpose::NativeResume)
}

#[derive(Clone, Copy)]
enum RootPurpose {
    LaunchIdentity,
    NativeResume,
}

struct SessionIdentity {
    target: SessionResumeTarget,
    parent: Option<String>,
    root_hint: Option<String>,
    parent_owned: bool,
}

fn resolve_root(
    home: &Path,
    id: &str,
    purpose: RootPurpose,
) -> Result<Option<SessionResumeTarget>> {
    let Some(id) = full_codex_session_id(id) else {
        return Ok(None);
    };
    let mut current = id.to_ascii_lowercase();
    let mut visited = BTreeSet::new();
    let mut expected_root = None;
    for depth in 0..MAX_PARENT_DEPTH {
        ensure!(
            visited.insert(current.clone()),
            "session parent ancestry contains a cycle"
        );
        let Some(identity) = read_identity(home, &current)? else {
            if depth == 0 {
                return Ok(None);
            }
            bail!("session parent metadata is missing; restore the parent session before resuming");
        };
        if depth == 0 && matches!(purpose, RootPurpose::NativeResume) && !identity.parent_owned {
            return Ok(Some(identity.target));
        }
        if let Some(hint) = identity.root_hint.as_ref().filter(|hint| **hint != current) {
            if let Some(expected) = &expected_root {
                ensure!(
                    expected == hint,
                    "session ancestry has conflicting root identities"
                );
            } else {
                expected_root = Some(hint.clone());
            }
        }
        if let Some(parent) = identity.parent {
            current = parent;
            continue;
        }
        ensure!(
            expected_root
                .as_ref()
                .is_none_or(|expected| expected == &identity.target.id),
            "session ancestry does not reach its recorded root"
        );
        return Ok(Some(identity.target));
    }
    bail!("session parent ancestry exceeds the safe depth limit")
}

fn read_identity(home: &Path, id: &str) -> Result<Option<SessionIdentity>> {
    let mut paths = Vec::new();
    collect_exact_session_paths_for_selector(home, id, &mut paths)?;
    paths.sort();
    paths.dedup();
    let mut identity = None;
    let mut seen_paths = BTreeSet::new();
    for path in paths {
        let canonical = match path.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("failed to resolve session metadata path"),
        };
        if !seen_paths.insert(canonical) {
            continue;
        }
        let mut header = None;
        visit_session_lines(&path, |line| {
            if line.trim().is_empty() {
                return true;
            }
            header = Some(serde_json::from_str::<serde_json::Value>(line));
            false
        })?;
        // A missing/corrupt prefix still belongs to the existing metadata-repair
        // path. A known child whose parent lacks a valid header fails above.
        let Some(Ok(header)) = header else {
            continue;
        };
        let metadata = session_value_metadata(&header);
        if !matches!(metadata.type_class, 0 | 1)
            || metadata
                .resume_id
                .as_deref()
                .is_none_or(|value| !value.eq_ignore_ascii_case(id))
        {
            continue;
        }
        ensure!(
            identity.is_none(),
            "session identity is ambiguous; conflicting rollout files exist"
        );
        let fields = header
            .get("payload")
            .filter(|value| value.is_object())
            .unwrap_or(&header);
        let parent = metadata
            .parent_thread_id
            .map(|value| validated_id(&value))
            .transpose()?;
        // Reject malformed parent metadata instead of silently treating a child as a root.
        for value in [
            fields.get("parent_thread_id"),
            fields.pointer("/source/subagent/thread_spawn/parent_thread_id"),
        ]
        .into_iter()
        .flatten()
        {
            if !value.is_null() {
                let value =
                    validated_id(value.as_str().context("invalid session parent identity")?)?;
                ensure!(
                    parent.as_deref() == Some(&value),
                    "session parent identities disagree"
                );
            }
        }
        let root_hint = fields
            .get("session_id")
            .filter(|value| !value.is_null())
            .map(|value| validated_id(value.as_str().context("invalid session root identity")?))
            .transpose()?;
        let parent_owned = parent.is_some()
            && fields
                .get("multi_agent_version")
                .and_then(serde_json::Value::as_str)
                == Some("v2");
        identity = Some(SessionIdentity {
            target: SessionResumeTarget {
                id: id.to_owned(),
                path,
            },
            parent,
            root_hint,
            parent_owned,
        });
    }
    Ok(identity)
}

fn validated_id(value: &str) -> Result<String> {
    ensure!(
        full_codex_session_id(value).is_some(),
        "invalid session ancestry UUID"
    );
    Ok(value.to_ascii_lowercase())
}
