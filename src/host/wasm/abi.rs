//! The raw `extern "C"` ABI shared by both halves (design 12.8.10).
//!
//! JS writes input bytes into memory it got from `alloc` and reads output
//! from the outbox: one byte buffer of framed records, each `[u32 LE
//! length][record]`. A record starts with its tag byte: the `host::wire`
//! tags, the browser install records of `dsp::ring`, and the extension tags
//! below. JS never parses a record; it only reads the frame lengths to move
//! each record as its own `ArrayBuffer`, and routes `TAG_CONSOLE` to the page.
//!
//! Each wasm instance runs one half on one thread, so the state of a half is
//! a `thread_local!` `RefCell` (no `static mut`). The worklet half reserves
//! the outbox capacity at init and never grows it (a full outbox drops the
//! record and counts it), so its memory stays unchanged after init (16.1).

use std::cell::{Cell, RefCell};

/// Main -> page: UTF-8 text of a diagnostic, failure or printed line.
pub const TAG_CONSOLE: u8 = 0x70;
/// Worklet -> main: an install fault, `[code u8][resource u32]`.
pub const TAG_FAULT: u8 = 0x60;
/// Worklet -> main: the host signals, `[amp f32][fft 8 x f32]`.
pub const TAG_SIGS: u8 = 0x61;
/// Session -> page: one Session Protocol v1 server envelope, or the
/// browser-local `check` record, as UTF-8 JSON (design 15.1.2 G1).
pub const TAG_SESSION: u8 = 0x71;
/// Session -> page: a render record (`set_program`/`set_uniforms`) as
/// UTF-8 JSON (design 15.1.2 G1).
pub const TAG_RENDER: u8 = 0x72;
/// Session -> page: a package driver reply as UTF-8 JSON (design 15.1.2
/// G6).
pub const TAG_PKG: u8 = 0x73;

/// The frame header: the record length.
pub const FRAME_LEN: usize = 4;

thread_local! {
    static OUTBOX: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Zero: the outbox may grow (main half). Otherwise its fixed capacity.
    static OUTBOX_CAP: Cell<usize> = const { Cell::new(0) };
    static OUTBOX_DROPPED: Cell<u64> = const { Cell::new(0) };
}

/// Reserves `bytes` of outbox and fixes the capacity (worklet init).
pub fn fix_outbox(bytes: usize) {
    OUTBOX.with(|o| {
        let mut o = o.borrow_mut();
        o.clear();
        o.reserve_exact(bytes);
    });
    OUTBOX_CAP.with(|c| c.set(bytes));
}

/// Whether a whole record fits an empty outbox; zero capacity permits growth.
/// This does not modify the queue or its refusal counter.
pub(crate) fn record_fits_empty_outbox(record_bytes: usize) -> bool {
    if u32::try_from(record_bytes).is_err() {
        return false;
    }
    let Some(framed) = FRAME_LEN.checked_add(record_bytes) else {
        return false;
    };
    let cap = OUTBOX_CAP.with(Cell::get);
    cap == 0 || framed <= cap
}

/// Appends one framed record; false (and counted) when a fixed outbox has
/// no room for it.
pub fn push_record(rec: &[u8]) -> bool {
    let Ok(len) = u32::try_from(rec.len()) else {
        OUTBOX_DROPPED.with(|d| d.set(d.get() + 1));
        return false;
    };
    let cap = OUTBOX_CAP.with(Cell::get);
    OUTBOX.with(|o| {
        let mut o = o.borrow_mut();
        if cap > 0 && o.len() + FRAME_LEN + rec.len() > cap {
            OUTBOX_DROPPED.with(|d| d.set(d.get() + 1));
            return false;
        }
        o.extend_from_slice(&len.to_le_bytes());
        o.extend_from_slice(rec);
        true
    })
}

/// Records a fixed outbox refused.
pub fn outbox_dropped() -> u64 {
    OUTBOX_DROPPED.with(Cell::get)
}

/// Calls `f` for every framed record of `bytes` (a truncated tail is
/// ignored).
pub fn for_each_record(bytes: &[u8], mut f: impl FnMut(&[u8])) {
    let mut at = 0;
    while let Some(head) = bytes.get(at..at + FRAME_LEN) {
        let n = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
        let Some(rec) = bytes.get(at + FRAME_LEN..at + FRAME_LEN + n) else {
            return;
        };
        f(rec);
        at += FRAME_LEN + n;
    }
}

/// Copies `len` bytes JS wrote at `ptr` (memory from `alloc`).
///
/// # Safety
/// `ptr..ptr + len` must be readable memory of this instance, or `len` 0.
pub unsafe fn input<'a>(ptr: *const u8, len: u32) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        return &[];
    }
    // SAFETY: the caller guarantees the range is live memory JS wrote.
    unsafe { std::slice::from_raw_parts(ptr, len as usize) }
}

/// Allocates `len` bytes for JS to write into; free with `free`.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let mut v = vec![0u8; (len as usize).max(1)].into_boxed_slice();
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Frees memory from `alloc`.
///
/// # Safety
/// `ptr` and `len` must be exactly an earlier `alloc` result and its length.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub unsafe extern "C" fn free(ptr: *mut u8, len: u32) {
    if ptr.is_null() {
        return;
    }
    let n = (len as usize).max(1);
    // SAFETY: `alloc` leaked a boxed slice of exactly `n` bytes at `ptr`.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, n)) });
}

/// The start of the outbox.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn outbox_ptr() -> *const u8 {
    OUTBOX.with(|o| o.borrow().as_ptr())
}

/// The outbox length in bytes.
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn outbox_len() -> u32 {
    OUTBOX.with(|o| u32::try_from(o.borrow().len()).unwrap_or(u32::MAX))
}

/// Empties the outbox (its capacity is kept).
#[cfg_attr(target_arch = "wasm32", no_mangle)]
pub extern "C" fn outbox_clear() {
    OUTBOX.with(|o| o.borrow_mut().clear());
}
