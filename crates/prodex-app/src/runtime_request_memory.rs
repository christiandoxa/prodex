//! Process-wide admission for the memory retained by captured request bodies.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "linux")]
mod linux_cgroup;
#[cfg(windows)]
mod windows_job;

// Capture, JSON inspection, provider transforms and retry can coexist in memory.
const REQUEST_MEMORY_MULTIPLIER: u64 = 8;
const FALLBACK_AVAILABLE_MEMORY: u64 = 512 * 1024 * 1024;

static REQUEST_MEMORY: OnceLock<RequestMemoryBudget> = OnceLock::new();

pub(crate) struct RequestMemoryBudget {
    available: AtomicU64,
    reserved: AtomicU64,
}

pub(crate) struct RequestMemoryGuard<'a> {
    budget: &'a RequestMemoryBudget,
    reserved: u64,
}

#[derive(Debug)]
pub(crate) struct RequestMemoryExhausted;

impl std::fmt::Display for RequestMemoryExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("runtime request memory is temporarily unavailable")
    }
}

impl std::error::Error for RequestMemoryExhausted {}

impl RequestMemoryBudget {
    pub(crate) fn new(available: u64) -> Self {
        Self {
            available: AtomicU64::new(available),
            reserved: AtomicU64::new(0),
        }
    }

    pub(crate) fn guard(&self) -> RequestMemoryGuard<'_> {
        RequestMemoryGuard {
            budget: self,
            reserved: 0,
        }
    }
}

impl RequestMemoryGuard<'_> {
    pub(crate) fn reserve_body_bytes(
        &mut self,
        body_bytes: u64,
    ) -> Result<(), RequestMemoryExhausted> {
        let needed = body_bytes
            .checked_mul(REQUEST_MEMORY_MULTIPLIER)
            .ok_or(RequestMemoryExhausted)?;
        let additional = needed.saturating_sub(self.reserved);
        self.budget
            .reserved
            .try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(additional)
                    .filter(|total| *total <= self.budget.available.load(Ordering::Acquire))
            })
            .map_err(|_| RequestMemoryExhausted)?;
        self.reserved += additional;
        Ok(())
    }
}

impl Drop for RequestMemoryGuard<'_> {
    fn drop(&mut self) {
        self.budget
            .reserved
            .fetch_sub(self.reserved, Ordering::AcqRel);
    }
}

/// Sample host and container capacity before request workers start.
pub(crate) fn initialize_request_memory_budget() {
    let available = available_memory().unwrap_or(FALLBACK_AVAILABLE_MEMORY);
    let budget = REQUEST_MEMORY.get_or_init(|| RequestMemoryBudget::new(available));
    budget.available.store(available, Ordering::Release);
}

pub(crate) fn spawn_request_memory_sampler(
    shutdown: Arc<AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("prodex-request-memory".to_string())
        .spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                std::thread::park_timeout(std::time::Duration::from_secs(2));
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                if let Some(budget) = REQUEST_MEMORY.get() {
                    budget.available.store(
                        available_memory().unwrap_or(FALLBACK_AVAILABLE_MEMORY),
                        Ordering::Release,
                    );
                }
            }
        })
}

pub(crate) fn request_memory_guard() -> Result<RequestMemoryGuard<'static>, RequestMemoryExhausted>
{
    REQUEST_MEMORY
        .get()
        .map(RequestMemoryBudget::guard)
        .ok_or(RequestMemoryExhausted)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn physical_memory() -> Option<u64> {
    // SAFETY: sysconf takes constant selectors and does not dereference pointers.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    // SAFETY: the page-size selector has the same sysconf contract.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    u64::try_from(pages)
        .ok()?
        .checked_mul(u64::try_from(page_size).ok()?)
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_host_self() -> libc::mach_port_t;
    static mut mach_task_self_: libc::mach_port_t;
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;
}

#[cfg(target_os = "macos")]
fn physical_memory() -> Option<u64> {
    let mut statistics = std::mem::MaybeUninit::<libc::vm_statistics64>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: the output buffer and count describe a writable vm_statistics64;
    // the acquired host port is released on both success and failure.
    let result = unsafe {
        let host = mach_host_self();
        let status = libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            statistics.as_mut_ptr().cast(),
            &mut count,
        );
        let _ = mach_port_deallocate(mach_task_self_, host);
        status
    };
    if result != libc::KERN_SUCCESS || count < 3 {
        return None;
    }
    // SAFETY: the buffer was zero-initialized before the kernel filled it.
    let statistics = unsafe { statistics.assume_init() };
    // SAFETY: sysconf takes a constant selector and does not dereference pointers.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    u64::from(statistics.free_count)
        .checked_add(u64::from(statistics.inactive_count))?
        .checked_mul(u64::try_from(page_size).ok()?)
}

#[cfg(windows)]
fn physical_memory() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX::default();
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: status is a valid writable structure with its length initialized.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    let host = status.ullAvailPhys.min(status.ullAvailVirtual);
    Some(match windows_job::available() {
        Ok(Some(limit)) => host.min(limit),
        Ok(None) => host,
        Err(()) => host.min(FALLBACK_AVAILABLE_MEMORY),
    })
}

#[cfg(not(any(unix, windows)))]
fn physical_memory() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
fn available_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok();
    let host = meminfo
        .as_deref()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("MemAvailable:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()?
                    .checked_mul(1024)
            })
        })
        .or_else(physical_memory)?;
    let cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .and_then(|groups| {
            std::fs::read_to_string("/proc/self/mountinfo").map(|mounts| (groups, mounts))
        })
        .map_err(|_| ())
        .and_then(|(groups, mounts)| {
            linux_cgroup::available(&groups, &mounts, |path| std::fs::read_to_string(path))
        });
    Some(match cgroup {
        Ok(Some(limit)) => host.min(limit),
        Ok(None) => host,
        Err(()) => host.min(FALLBACK_AVAILABLE_MEMORY),
    })
}

#[cfg(not(target_os = "linux"))]
fn available_memory() -> Option<u64> {
    physical_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_memory_is_shared_and_released_on_drop() {
        let budget = RequestMemoryBudget::new(80);
        let mut first = budget.guard();
        let mut second = budget.guard();
        assert!(first.reserve_body_bytes(6).is_ok());
        assert!(second.reserve_body_bytes(5).is_err());
        assert!(second.reserve_body_bytes(4).is_ok());
        assert!(first.reserve_body_bytes(7).is_err());
        drop(second);
        assert!(first.reserve_body_bytes(10).is_ok());
        drop(first);
        assert_eq!(budget.reserved.load(Ordering::Acquire), 0);
        assert!(budget.guard().reserve_body_bytes(u64::MAX).is_err());
    }

    #[test]
    fn memory_reservation_tracks_updated_available_capacity() {
        let budget = RequestMemoryBudget::new(80);
        let mut guard = budget.guard();
        assert!(guard.reserve_body_bytes(4).is_ok());
        budget.available.store(39, Ordering::Release);
        assert!(guard.reserve_body_bytes(5).is_err());
        budget.available.store(40, Ordering::Release);
        assert!(guard.reserve_body_bytes(5).is_ok());
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[test]
    fn native_memory_capacity_is_detected() {
        assert!(available_memory().is_some_and(|bytes| bytes > 0));
    }

    #[test]
    fn concurrent_request_reservations_cannot_exceed_budget() {
        let budget = Arc::new(RequestMemoryBudget::new(80));
        let ready = Arc::new(std::sync::Barrier::new(11));
        let held = Arc::new(std::sync::Barrier::new(11));
        let accepted = AtomicU64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..10 {
                let budget = Arc::clone(&budget);
                let ready = Arc::clone(&ready);
                let held = Arc::clone(&held);
                let accepted = &accepted;
                scope.spawn(move || {
                    let mut guard = budget.guard();
                    ready.wait();
                    if guard.reserve_body_bytes(2).is_ok() {
                        accepted.fetch_add(1, Ordering::Relaxed);
                    }
                    held.wait();
                });
            }
            ready.wait();
            held.wait();
        });
        assert_eq!(accepted.load(Ordering::Relaxed), 5);
        assert_eq!(budget.reserved.load(Ordering::Acquire), 0);
    }

    #[test]
    fn sampler_stops_when_its_runtime_stops() {
        initialize_request_memory_budget();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker = spawn_request_memory_sampler(Arc::clone(&shutdown)).expect("sampler starts");
        shutdown.store(true, Ordering::Release);
        worker.thread().unpark();
        worker.join().expect("sampler stops");
    }
}
