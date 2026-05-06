use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateFacts {
    pub(super) branch: Option<String>,
    pub(super) dirty_files: Option<BTreeSet<String>>,
    pub(super) recent_changed_files: Option<BTreeSet<String>>,
    pub(super) package_managers: Option<BTreeSet<String>>,
    pub(super) main_test_commands: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextRepoStateFactRelation {
    New,
    Repeated,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextRepoStateListKind {
    Dirty,
    RecentChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateLineSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RuntimeSmartContextRepoStateTextObservation {
    pub(super) facts: RuntimeSmartContextRepoStateFacts,
    pub(super) spans: Vec<RuntimeSmartContextRepoStateLineSpan>,
}
