//! Raw wasm exports for formatting a source buffer (design 3.7.3).

use std::cell::RefCell;

thread_local! {
    static FMT_OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Formats the UTF-8 source at `ptr` and stores the result for the output accessors.
///
/// Returns the `STATUS_*` value from [`crate::fmt::format_bytes`]. Refused or
/// non-UTF-8 input is copied unchanged into the output buffer.
///
/// # Safety
/// `ptr..ptr + len` must be readable memory allocated by this wasm instance,
/// or `len` must be zero.
#[no_mangle]
pub unsafe extern "C" fn fmt_source(ptr: *const u8, len: u32) -> u32 {
    // SAFETY: this export's caller guarantees the input memory range is live.
    let input = unsafe { super::abi::input(ptr, len) };
    let (status, bytes) = crate::fmt::format_bytes(input);
    FMT_OUT.with(|output| *output.borrow_mut() = bytes);
    status
}

/// Returns the current formatted output buffer's address.
///
/// Read the output before calling `fmt_source` again, since that call may
/// reallocate the buffer. Refresh the JS `memory.buffer` after every call to
/// `fmt_source`, since wasm memory may have grown.
#[no_mangle]
pub extern "C" fn fmt_out_ptr() -> *const u8 {
    FMT_OUT.with(|output| output.borrow().as_ptr())
}

/// Returns the current formatted output buffer's length in bytes.
#[no_mangle]
pub extern "C" fn fmt_out_len() -> u32 {
    FMT_OUT.with(|output| u32::try_from(output.borrow().len()).unwrap_or(u32::MAX))
}
