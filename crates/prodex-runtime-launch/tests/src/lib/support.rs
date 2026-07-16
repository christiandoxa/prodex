use super::*;

pub(super) fn test_profile(path: &str) -> prodex_state::ProfileEntry {
    prodex_state::ProfileEntry {
        codex_home: PathBuf::from(path),
        managed: true,
        email: None,
        provider: prodex_state::ProfileProvider::Openai,
    }
}
