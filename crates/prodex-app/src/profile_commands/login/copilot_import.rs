use super::super::handle_import_copilot_profile;
use crate::ImportProfileArgs;
use anyhow::Result;
use std::path::PathBuf;

pub(super) fn handle_copilot_login_import() -> Result<()> {
    handle_import_copilot_profile(&ImportProfileArgs {
        path: PathBuf::from("copilot"),
        name: None,
        activate: false,
    })
}
