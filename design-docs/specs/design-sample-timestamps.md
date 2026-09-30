# Sample timestamps in milliseconds

## Public syntax

Add pattern controls `start-ms` and `stop-ms`. Integer 123 denotes 123 milliseconds; decimal 123.456 denotes 123 milliseconds plus 456 microseconds. Zero is valid; negative, nonfinite, overflow and finer-than-microsecond values report diagnostics. One or two decimal places are accepted with implied trailing zeros. The stop boundary is exclusive; omitted start is zero and omitted stop is the loaded sample's end.

Use the actual downloaded full WAV in the breakcore score. Chopping, indexing, rearrangement, speed changes, retriggering and reversals must be expressed in `.vact`; do not make externally cut sample files the composition's mechanism.

## Representation and composition

Convert numeric notation once per resolved event into checked integer microseconds. Keep region arithmetic exact using integer or rational values; convert to DSP coordinates only at the final boundary, with frame-level precision governed by source sample rate. Do not introduce accumulating floating-point timestamp arithmetic. Work for static values, lists, alternation and dynamically resolved numeric patterns. Existing normalized `begin`/`end` and `chop`/`slice`/`splice` apply within the absolute timestamp window, so both styles compose predictably. `speed-fit` must use the selected window's duration.

Reject invalid or inverted windows, non-sample uses, missing samples and values outside the loaded source. Preserve legacy normalized region behavior when no timestamp window is supplied. Use existing architecture and no external dependency. If the existing final normalized f32 wire cannot preserve frame selection for supported long sources, address that conversion rather than claiming precision from input notation alone.

## Verification

Check integer milliseconds, decimals to three places, decimal shorthand, one-microsecond steps, precision rejection, negative/NaN/infinity/overflow, omitted bounds, inverted/out-of-source windows, composition with chop/slice/splice/speed-fit/reverse and actual sampled audio at 44.1/48/96 kHz. Verify the feature through native and browser runtime paths. Keep touched Rust below 1000 lines.

## Example

` s {sample ./amen.wav} > start-ms 123.456 > stop-ms 487 > chop 8 `

See [sampled breakcore](design-genre-tracks.md#sampled-breakcore-addition) for the CC0 source and arrangement requirements.

## User examples

- [Timestamp and chop demo](../../examples/sample-timestamps.vact) shows four editing approaches over one source WAV.
- [Glass Teeth](../../examples/tracks/breakcore/glass-teeth.vact) is the complete breakcore arrangement.

Timestamp argument literal trees preserve exact reader ratios through lists and
arithmetic blocks. Float32 variables computed elsewhere have already lost any
unrepresentable digits; timestamp operators cannot restore those. Prefer
integer/ratio values for such variables when exact timing matters.
