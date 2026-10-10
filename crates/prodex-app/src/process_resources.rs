//! Best-effort process resource headroom before state I/O and worker startup.
//! This raises only the inherited soft descriptor limit; it never changes the
//! hard limit, closes somebody else's descriptors, or substitutes empty state.

#[cfg(unix)]
const DESCRIPTOR_SOFT_TARGET: libc::rlim_t = 8_192;

pub(crate) fn prepare_descriptor_headroom() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut limits = std::mem::MaybeUninit::<libc::rlimit>::uninit();
        // SAFETY: getrlimit initializes a correctly sized, writable rlimit.
        if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limits.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a successful getrlimit initialized the entire value.
        let mut limits = unsafe { limits.assume_init() };
        let target = DESCRIPTOR_SOFT_TARGET.min(limits.rlim_max);
        if limits.rlim_cur >= target {
            return Ok(());
        }
        limits.rlim_cur = target;
        // SAFETY: only this process's soft limit changes, within its hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limits) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
