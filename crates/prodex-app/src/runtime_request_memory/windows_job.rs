use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::System::JobObjects::{
    IsProcessInJob, JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    QueryInformationJobObject,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

// This information class is used by Microsoft's Windows container runtime.
const JOB_OBJECT_MEMORY_USAGE_INFORMATION: i32 = 28;

#[repr(C)]
#[derive(Default)]
struct JobMemoryUsage {
    current: u64,
    peak: u64,
}

pub(super) fn available() -> Result<Option<u64>, ()> {
    let mut in_job = 0;
    // SAFETY: the pseudo-handle is valid and in_job is writable.
    let process = unsafe { GetCurrentProcess() };
    if unsafe { IsProcessInJob(process, null_mut(), &mut in_job) } == 0 {
        return Err(());
    }
    if in_job == 0 {
        return Ok(None);
    }
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    // SAFETY: the output structure and size match the requested information class.
    if unsafe {
        QueryInformationJobObject(
            null_mut(),
            JobObjectExtendedLimitInformation,
            (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<c_void>(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            null_mut(),
        )
    } == 0
    {
        return Err(());
    }
    let flags = limits.BasicLimitInformation.LimitFlags;
    let mut available: Option<u64> = None;
    if flags & JOB_OBJECT_LIMIT_JOB_MEMORY != 0 {
        let mut usage = JobMemoryUsage::default();
        // SAFETY: class 28 accepts this two-u64 structure, matching the Windows ABI.
        if unsafe {
            QueryInformationJobObject(
                null_mut(),
                JOB_OBJECT_MEMORY_USAGE_INFORMATION,
                (&mut usage as *mut JobMemoryUsage).cast::<c_void>(),
                std::mem::size_of::<JobMemoryUsage>() as u32,
                null_mut(),
            )
        } == 0
        {
            return Err(());
        }
        available = Some((limits.JobMemoryLimit as u64).saturating_sub(usage.current));
    }
    if flags & JOB_OBJECT_LIMIT_PROCESS_MEMORY != 0 {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        // SAFETY: the extended counters begin with PROCESS_MEMORY_COUNTERS and cb describes the full buffer.
        if unsafe {
            K32GetProcessMemoryInfo(
                process,
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                counters.cb,
            )
        } == 0
        {
            return Err(());
        }
        let remaining =
            (limits.ProcessMemoryLimit as u64).saturating_sub(counters.PrivateUsage as u64);
        available = Some(available.map_or(remaining, |previous| previous.min(remaining)));
    }
    Ok(available)
}
