//! The allocation probe (design 12.8.9), test builds only.
//!
//! `Counting` is the crate's global allocator under `#[cfg(test)]`
//! (`src/lib.rs`). It forwards to `System` and counts `alloc`,
//! `alloc_zeroed` and `realloc` on the current thread while that thread is
//! armed. The thread-locals are `const`-initialized, so arming and counting
//! allocate nothing themselves. Headless render tests wrap every
//! `Engine::process` call in `armed` and assert a zero count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

/// A `System` allocator that counts allocations on armed threads.
pub struct Counting;

fn note() {
    // `try_with`: allocations during thread-local teardown are not counted
    // and never panic.
    if ARMED.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

// SAFETY: every method forwards to `System` with the caller's arguments
// unchanged; counting touches only `const` thread-locals, which never
// allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }
}

/// Runs `f` with the current thread armed; returns its result and the
/// number of allocations it made. Nested calls restore the outer state.
pub fn armed<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let was = ARMED.with(|a| a.replace(true));
    let before = COUNT.with(Cell::get);
    let r = f();
    let after = COUNT.with(Cell::get);
    ARMED.with(|a| a.set(was));
    (r, after - before)
}
