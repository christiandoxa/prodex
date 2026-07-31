use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::IMPORT_AUTH_UPDATE_JOURNAL_VERSION;

#[derive(Clone, PartialEq, Eq)]
pub struct ImportedExistingProfileAuthUpdate {
    pub profile_name: String,
    pub codex_home: PathBuf,
    pub previous_auth_json: Option<String>,
    pub previous_email: Option<String>,
    pub journal_path: Option<PathBuf>,
    pub restore_auth_json: bool,
    pub previous_provider_json: Option<String>,
    pub previous_secret_files: Vec<ImportedExistingProfileFileRollback>,
}

impl fmt::Debug for ImportedExistingProfileAuthUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedExistingProfileAuthUpdate")
            .field("profile_name", &self.profile_name)
            .field("codex_home", &self.codex_home)
            .field("previous_auth_json", &"<redacted>")
            .field("previous_email", &self.previous_email)
            .field("journal_path", &self.journal_path)
            .field("restore_auth_json", &self.restore_auth_json)
            .field("previous_provider_json", &"<redacted>")
            .field("previous_secret_files", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedExistingProfileFileRollback {
    pub path: String,
    pub previous_text: Option<String>,
}

impl fmt::Debug for ImportedExistingProfileFileRollback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedExistingProfileFileRollback")
            .field("path", &self.path)
            .field("previous_text", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedExistingProfileFileUpdate {
    pub path: String,
    pub text: Option<String>,
}

impl fmt::Debug for ImportedExistingProfileFileUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedExistingProfileFileUpdate")
            .field("path", &self.path)
            .field("text", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedProfilesCommit {
    pub imported_names: Vec<String>,
    pub updated_existing_names: Vec<String>,
    pub committed_homes: Vec<PathBuf>,
    pub auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    pub previous_active_profile: Option<String>,
    pub lifecycle_journal_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedProfilesTransaction {
    pub imported_names: Vec<String>,
    pub updated_existing_names: Vec<String>,
    pub committed_homes: Vec<PathBuf>,
    pub auth_updates: Vec<ImportedExistingProfileAuthUpdate>,
    pub previous_active_profile: Option<String>,
    pub lifecycle_journal_path: Option<PathBuf>,
}

impl ImportedProfilesTransaction {
    pub fn new(
        previous_active_profile: Option<String>,
        staged_profile_count: usize,
        auth_update_count: usize,
    ) -> Self {
        Self {
            imported_names: Vec::with_capacity(staged_profile_count),
            updated_existing_names: Vec::with_capacity(auth_update_count),
            committed_homes: Vec::with_capacity(staged_profile_count),
            auth_updates: Vec::with_capacity(auth_update_count),
            previous_active_profile,
            lifecycle_journal_path: None,
        }
    }

    pub fn set_lifecycle_journal_path(&mut self, path: PathBuf) {
        self.lifecycle_journal_path = Some(path);
    }

    pub fn record_existing_auth_update(&mut self, update: ImportedExistingProfileAuthUpdate) {
        self.updated_existing_names
            .push(update.profile_name.clone());
        self.auth_updates.push(update);
    }

    pub fn record_imported_profile(&mut self, name: String, final_home: PathBuf) {
        self.committed_homes.push(final_home);
        self.imported_names.push(name);
    }

    pub fn into_commit(self) -> ImportedProfilesCommit {
        ImportedProfilesCommit {
            imported_names: self.imported_names,
            updated_existing_names: self.updated_existing_names,
            committed_homes: self.committed_homes,
            auth_updates: self.auth_updates,
            previous_active_profile: self.previous_active_profile,
            lifecycle_journal_path: self.lifecycle_journal_path,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedExistingProfileAuthUpdateJournal {
    pub version: u32,
    pub profile_name: String,
    pub codex_home: String,
    pub previous_email: Option<String>,
    pub previous_auth_json: Option<String>,
    #[serde(default = "journal_restore_auth_json_default")]
    pub restore_auth_json: bool,
    #[serde(default)]
    pub previous_provider_json: Option<String>,
    #[serde(default)]
    pub previous_secret_files: Vec<ImportedExistingProfileFileRollback>,
    #[serde(default)]
    pub state_after_known: bool,
    #[serde(default)]
    pub next_email: Option<String>,
    #[serde(default)]
    pub next_auth_json: Option<String>,
    #[serde(default)]
    pub next_provider_json: Option<String>,
    #[serde(default)]
    pub next_secret_files: Vec<ImportedExistingProfileFileUpdate>,
    #[serde(default)]
    pub temporary_home: Option<String>,
    pub created_at: String,
}

impl fmt::Debug for ImportedExistingProfileAuthUpdateJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportedExistingProfileAuthUpdateJournal")
            .field("version", &self.version)
            .field("profile_name", &self.profile_name)
            .field("codex_home", &self.codex_home)
            .field("previous_email", &self.previous_email)
            .field("previous_auth_json", &"<redacted>")
            .field("restore_auth_json", &self.restore_auth_json)
            .field("previous_provider_json", &"<redacted>")
            .field("previous_secret_files", &"<redacted>")
            .field("state_after_known", &self.state_after_known)
            .field("next_email", &self.next_email)
            .field("next_auth_json", &"<redacted>")
            .field("next_provider_json", &"<redacted>")
            .field("next_secret_files", &"<redacted>")
            .field("temporary_home", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl ImportedExistingProfileAuthUpdateJournal {
    pub fn new(
        profile_name: String,
        codex_home: String,
        previous_email: Option<String>,
        previous_auth_json: Option<String>,
        created_at: String,
    ) -> Self {
        Self {
            version: IMPORT_AUTH_UPDATE_JOURNAL_VERSION,
            profile_name,
            codex_home,
            previous_email,
            previous_auth_json,
            restore_auth_json: true,
            previous_provider_json: None,
            previous_secret_files: Vec::new(),
            state_after_known: false,
            next_email: None,
            next_auth_json: None,
            next_provider_json: None,
            next_secret_files: Vec::new(),
            temporary_home: None,
            created_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileLifecycleJournal {
    pub version: u32,
    pub operation: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

impl fmt::Debug for ProfileLifecycleJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileLifecycleJournal")
            .field("version", &self.version)
            .field("operation", &self.operation)
            .field("payload", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl ProfileLifecycleJournal {
    pub fn new(operation: String, payload: serde_json::Value, created_at: String) -> Self {
        Self {
            version: crate::PROFILE_LIFECYCLE_JOURNAL_VERSION,
            operation,
            payload,
            created_at,
        }
    }
}

fn journal_restore_auth_json_default() -> bool {
    true
}
