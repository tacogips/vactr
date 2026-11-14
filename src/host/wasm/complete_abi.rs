//! Raw wasm exports for context-aware completion (design 5.1).

use std::cell::RefCell;

thread_local! {
    static COMPLETE_OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Completes the UTF-8 source at `ptr` and stores the JSON result for the output accessors.
///
/// Returns the `STATUS_*` value from [`crate::complete::complete_bytes`]. The output remains
/// valid until the next call to `complete_source`. JavaScript callers must refresh
/// `memory.buffer` after each call to this function before reading the output.
///
/// # Safety
/// `ptr..ptr + len` must be readable memory allocated by this wasm instance, or `len` must be
/// zero.
#[no_mangle]
pub unsafe extern "C" fn complete_source(ptr: *const u8, len: u32, cursor: u32, limit: u32) -> u32 {
    // SAFETY: this export's caller guarantees the input memory range is live.
    let input = unsafe { super::abi::input(ptr, len) };
    let (status, bytes) = crate::complete::complete_bytes(input, cursor, limit);
    COMPLETE_OUT.with(|output| *output.borrow_mut() = bytes);
    status
}

/// Returns the current completion result buffer's address.
///
/// The output remains valid until the next call to `complete_source`. JavaScript callers must
/// refresh `memory.buffer` after each call to `complete_source` before reading the output.
#[no_mangle]
pub extern "C" fn complete_out_ptr() -> *const u8 {
    COMPLETE_OUT.with(|output| output.borrow().as_ptr())
}

/// Returns the current completion result buffer's length in bytes.
#[no_mangle]
pub extern "C" fn complete_out_len() -> u32 {
    COMPLETE_OUT.with(|output| u32::try_from(output.borrow().len()).unwrap_or(u32::MAX))
}
