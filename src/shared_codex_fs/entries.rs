#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedCodexEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SharedCodexEntry {
    pub(super) name: String,
    pub(super) kind: SharedCodexEntryKind,
}

impl SharedCodexEntry {
    pub(super) fn directory(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: SharedCodexEntryKind::Directory,
        }
    }

    pub(super) fn file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SharedCodexEntryKind::File,
        }
    }
}
