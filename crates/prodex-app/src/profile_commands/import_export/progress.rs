use anyhow::Result;

use crate::print_stderr_line;

pub(super) fn print_profile_import_progress(message: &str) -> Result<()> {
    print_stderr_line(message).map_err(Into::into)
}
