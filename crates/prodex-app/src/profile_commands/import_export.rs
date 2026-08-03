use super::*;

mod export;
mod import;
mod lifecycle;
mod passwords;
mod progress;
mod secrets;

#[cfg(test)]
pub(super) use self::export::build_profile_export_payload;
pub(crate) use self::export::handle_export_profiles;
#[cfg(test)]
pub(super) use self::import::import_profile_export_payload;
#[cfg(test)]
pub(crate) use self::import::lifecycle_support::load_profile_state_with_profile_recovery;
pub(crate) use self::import::lifecycle_support::{
    load_profile_state_with_profile_recovery_locked, recover_pending_profile_lifecycle,
    repair_profile_import_auth_journals,
};
pub(crate) use self::import::{
    count_profile_import_auth_journals, handle_import_current_profile, handle_import_profiles,
};
#[cfg(test)]
pub(crate) use self::lifecycle::profile_lifecycle_lock_path;
#[cfg(test)]
pub(crate) use self::lifecycle::recover_profile_lifecycle_journals;
pub(crate) use self::lifecycle::{
    ProfileAuthUpdate, ProfileLifecycleHomeAction, ProfileLifecyclePlan,
    ProfileLifecyclePromoteRollback, acquire_profile_lifecycle_lock,
    cleanup_profile_lifecycle_and_auth_journal, lifecycle_profile_state,
    prepare_existing_profile_lifecycle, remove_home, write_profile_lifecycle_plan,
};
pub(crate) use self::secrets::{
    read_optional_secret_text_file, write_imported_auth_update_journal, write_secret_text_file,
};
