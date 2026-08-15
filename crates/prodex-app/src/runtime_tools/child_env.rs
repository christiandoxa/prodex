use super::ChildProcessPlan;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) fn clear_rtk_auto_wrap_control_env(child: &mut ChildProcessPlan) {
    let mut removed = BTreeSet::<OsString>::from_iter(child.removed_env.iter().cloned());
    removed.insert(OsString::from("PRODEX_RTK_AUTO_WRAP_DEPTH"));
    removed.insert(OsString::from("PRODEX_RTK_DISABLE_AUTO_WRAP"));
    child.removed_env = removed.into_iter().collect();
}

pub(crate) fn prepend_child_path(child: &mut ChildProcessPlan, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    let mut paths = vec![path];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        child.extra_env.push((OsString::from("PATH"), joined));
    }
}
