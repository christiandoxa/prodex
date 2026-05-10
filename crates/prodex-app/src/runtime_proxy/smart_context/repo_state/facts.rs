use super::*;

pub(super) fn runtime_smart_context_repo_state_compact_commands(
    commands: &BTreeSet<String>,
) -> String {
    let joined = commands.iter().cloned().collect::<Vec<_>>().join("; ");
    if joined.len() <= SMART_CONTEXT_REPO_STATE_TEST_COMMAND_MAX_CHARS {
        joined
    } else {
        format!(
            "{} cmds h={}",
            commands.len(),
            runtime_smart_context_short_hash(&joined, SMART_CONTEXT_REPO_STATE_HASH_CHARS)
        )
    }
}

pub(super) fn runtime_smart_context_repo_state_facts_short_hash(
    facts: &RuntimeSmartContextRepoStateFacts,
) -> String {
    runtime_smart_context_short_hash(&facts.canonical_text(), SMART_CONTEXT_REPO_STATE_HASH_CHARS)
}

fn runtime_smart_context_short_hash(text: &str, chars: usize) -> String {
    let hash = runtime_proxy_crate::smart_context_hash_text(text);
    hash.strip_prefix("sc:")
        .unwrap_or(hash.as_str())
        .chars()
        .take(chars)
        .collect()
}

impl RuntimeSmartContextRepoStateFacts {
    pub(super) fn is_empty(&self) -> bool {
        self.branch.is_none()
            && self.dirty_files.is_none()
            && self.recent_changed_files.is_none()
            && self.package_managers.is_none()
            && self.main_test_commands.is_none()
    }

    pub(super) fn merge_from(&mut self, other: &Self) {
        if other.branch.is_some() {
            self.branch = other.branch.clone();
        }
        if other.dirty_files.is_some() {
            self.dirty_files = other.dirty_files.clone();
        }
        if other.recent_changed_files.is_some() {
            self.recent_changed_files = other.recent_changed_files.clone();
        }
        if other.package_managers.is_some() {
            self.package_managers = other.package_managers.clone();
        }
        if other.main_test_commands.is_some() {
            self.main_test_commands = other.main_test_commands.clone();
        }
    }

    pub(super) fn relation_to(&self, cached: &Self) -> RuntimeSmartContextRepoStateFactRelation {
        let mut has_new = false;
        if runtime_smart_context_repo_state_compare_fact(
            self.branch.as_ref(),
            cached.branch.as_ref(),
            &mut has_new,
        )
        .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.dirty_files.as_ref(),
                cached.dirty_files.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.recent_changed_files.as_ref(),
                cached.recent_changed_files.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.package_managers.as_ref(),
                cached.package_managers.as_ref(),
                &mut has_new,
            )
            .is_none()
            || runtime_smart_context_repo_state_compare_fact(
                self.main_test_commands.as_ref(),
                cached.main_test_commands.as_ref(),
                &mut has_new,
            )
            .is_none()
        {
            return RuntimeSmartContextRepoStateFactRelation::Changed;
        }
        if has_new {
            RuntimeSmartContextRepoStateFactRelation::New
        } else {
            RuntimeSmartContextRepoStateFactRelation::Repeated
        }
    }

    fn canonical_text(&self) -> String {
        let mut lines = Vec::new();
        if let Some(branch) = self.branch.as_ref() {
            lines.push(format!("branch={branch}"));
        }
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "dirty",
            self.dirty_files.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "recent",
            self.recent_changed_files.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "pm",
            self.package_managers.as_ref(),
        );
        runtime_smart_context_repo_state_push_canonical_set(
            &mut lines,
            "test",
            self.main_test_commands.as_ref(),
        );
        lines.join("\n")
    }
}

fn runtime_smart_context_repo_state_compare_fact<T: Eq>(
    current: Option<&T>,
    cached: Option<&T>,
    has_new: &mut bool,
) -> Option<RuntimeSmartContextRepoStateFactRelation> {
    let Some(current) = current else {
        return Some(RuntimeSmartContextRepoStateFactRelation::Repeated);
    };
    let Some(cached) = cached else {
        *has_new = true;
        return Some(RuntimeSmartContextRepoStateFactRelation::New);
    };
    (current == cached).then_some(RuntimeSmartContextRepoStateFactRelation::Repeated)
}

fn runtime_smart_context_repo_state_push_canonical_set(
    lines: &mut Vec<String>,
    label: &str,
    values: Option<&BTreeSet<String>>,
) {
    if let Some(values) = values {
        lines.push(format!("{label}:"));
        lines.extend(values.iter().map(|value| format!("- {value}")));
    }
}
