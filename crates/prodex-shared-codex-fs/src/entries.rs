#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedCodexEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCodexEntry {
    pub name: String,
    pub kind: SharedCodexEntryKind,
}

impl SharedCodexEntry {
    pub fn directory(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: SharedCodexEntryKind::Directory,
        }
    }

    pub fn file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SharedCodexEntryKind::File,
        }
    }
}
