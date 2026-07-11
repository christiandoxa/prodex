//! Opt-in allocation counters for end-to-end benchmark evidence.
//!
//! The allocator is not enabled by this crate. A final binary must explicitly
//! select it with `#[global_allocator]`, which Prodex does only for the
//! `allocation-bench-support` feature.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static DEALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static REALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

/// A `System` allocator wrapper that records allocation operations.
///
/// # Safety contract
///
/// Every operation delegates to [`System`] with the exact pointer/layout pair
/// supplied by the caller. The wrapper neither dereferences nor retains
/// pointers. Counters use relaxed atomics because they are observational only;
/// they do not participate in allocator synchronization or program behavior.
pub struct CountingGlobalAllocator;

impl CountingGlobalAllocator {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CountingGlobalAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: the implementation preserves `GlobalAlloc`'s pointer/layout contract
// by forwarding each operation directly to `System`; see the type-level safety
// contract above. Counter updates do not inspect or alter allocated memory.
unsafe impl GlobalAlloc for CountingGlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is forwarded unchanged under `GlobalAlloc::alloc`'s
        // caller contract.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `layout` is forwarded unchanged under
        // `GlobalAlloc::alloc_zeroed`'s caller contract.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record_deallocation(layout.size());
        // SAFETY: the pointer/layout pair is forwarded unchanged under
        // `GlobalAlloc::dealloc`'s caller contract.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: all arguments are forwarded unchanged under
        // `GlobalAlloc::realloc`'s caller contract.
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        if !resized.is_null() {
            record_reallocation(layout.size(), new_size);
        }
        resized
    }
}

/// A process-wide cumulative allocation snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeAllocationSnapshot {
    pub alloc_calls: u64,
    pub realloc_calls: u64,
    pub dealloc_calls: u64,
    pub allocated_bytes: u64,
    pub reallocated_bytes: u64,
    pub deallocated_bytes: u64,
    pub live_bytes: u64,
    pub peak_live_bytes: u64,
}

impl RuntimeAllocationSnapshot {
    pub fn allocation_operations(self) -> u64 {
        self.alloc_calls.saturating_add(self.realloc_calls)
    }
}

pub fn runtime_allocation_snapshot() -> RuntimeAllocationSnapshot {
    RuntimeAllocationSnapshot {
        alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
        realloc_calls: REALLOC_CALLS.load(Ordering::Relaxed),
        dealloc_calls: DEALLOC_CALLS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        reallocated_bytes: REALLOCATED_BYTES.load(Ordering::Relaxed),
        deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
        live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn record_allocation(size: usize) {
    let size = size_u64(size);
    ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    add_live_bytes(size);
}

fn record_deallocation(size: usize) {
    let size = size_u64(size);
    DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
    DEALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    LIVE_BYTES.fetch_sub(size, Ordering::Relaxed);
}

fn record_reallocation(old_size: usize, new_size: usize) {
    let old_size = size_u64(old_size);
    let new_size = size_u64(new_size);
    REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
    REALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
    if new_size >= old_size {
        add_live_bytes(new_size - old_size);
    } else {
        LIVE_BYTES.fetch_sub(old_size - new_size, Ordering::Relaxed);
    }
}

fn add_live_bytes(size: u64) {
    let live = LIVE_BYTES
        .fetch_add(size, Ordering::Relaxed)
        .saturating_add(size);
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

fn size_u64(size: usize) -> u64 {
    u64::try_from(size).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: CountingGlobalAllocator = CountingGlobalAllocator::new();
