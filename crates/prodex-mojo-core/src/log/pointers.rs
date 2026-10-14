//! Caller-owned host-pointer address marshaling for log Mojo ABIs.

#[inline]
pub(super) fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

#[inline]
pub(super) fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}
