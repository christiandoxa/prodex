#[cfg(all(target_os = "linux", target_env = "gnu", not(test)))]
unsafe extern "C" {
    fn malloc_trim(pad: usize) -> i32;
}

/// Asks glibc to return free top-of-heap pages after bounded background work.
///
/// The caller treats this as a best-effort hint. Other allocators and test
/// builds intentionally do nothing.
pub(crate) fn runtime_allocator_trim_best_effort() -> bool {
    #[cfg(all(target_os = "linux", target_env = "gnu", not(test)))]
    {
        // SAFETY: glibc's malloc_trim accepts any padding value, has no pointer
        // arguments, and is called only on the supported libc/target pair.
        unsafe { malloc_trim(0) != 0 }
    }

    #[cfg(not(all(target_os = "linux", target_env = "gnu", not(test))))]
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn allocator_trim_is_disabled_in_test_builds() {
        assert!(!super::runtime_allocator_trim_best_effort());
    }
}
