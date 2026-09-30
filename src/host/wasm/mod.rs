//! The browser tier: the raw wasm ABI of both halves (design 12.8.10, 16).
//!
//! One `wasm32-unknown-unknown` module is instantiated twice with no shared
//! memory: on the main thread it runs the evaluator and the scheduler
//! (`main_half`), in the `AudioWorkletProcessor` only the DSP engine
//! (`worklet_half`). The module exports a raw `extern "C"` ABI (`abi`)
//! instead of wasm-bindgen, so the browser glue in `editor/worklet/` is plain
//! JS with no build step, and the core needs no JS imports. `messages` holds
//! the main half's hosts (`WasmAudioHost` with the 16.1 sender-paced slice
//! window, the cell port, the sample loader); `cells` the worklet's probed
//! cell mirror. Every record crossing between the halves is a `host::wire`
//! or `dsp::ring` byte record that JS moves as an `ArrayBuffer`. `session_half`
//! and `session_hosts` are the editor's session half: a page calls
//! `session_init` instead of `main_init` (design 15.1.2 G1).
//! The formatter's independent raw exports are `fmt_source`, `fmt_out_ptr`,
//! and `fmt_out_len` (`fmt_abi`).

pub mod abi;
pub mod cells;
pub mod fmt_abi;
pub mod main_half;
pub mod messages;
pub mod session_half;
pub mod session_hosts;
pub mod worklet_half;
