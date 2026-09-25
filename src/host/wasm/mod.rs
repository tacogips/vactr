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
//! or `dsp::ring` byte record that JS moves as an `ArrayBuffer`.

pub mod abi;
pub mod cells;
pub mod main_half;
pub mod messages;
pub mod worklet_half;
