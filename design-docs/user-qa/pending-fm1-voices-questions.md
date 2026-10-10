# Pending: six-operator FM extension and new voices

Decisions for [`design-fm1-voices.md`](../specs/design-fm1-voices.md)
(2026-10-10, session 347).

All seven items are decided. On 2026-10-10 (continuation run 2, session 352)
the owner accepted every recommendation: FV1 (a), FV2 (a), FV3 (the
exclusions), FV4 (a), FV5 (a), FV6 and FV7. No question is open.

## FV1. Legacy value for the `fm` template's `algorithm`

**Decided (owner, 2026-10-10): (a).** Algorithm 0 is the legacy
two-operator stack.

**Question.** The golden graph digest of `fm` must not change, so the body
cannot pass `algorithm:`. Which value means "legacy two-operator stack"?

- (a) **Chosen.** `algorithm 0` is legacy.
  - The `fm` header default changes from 5 to 0.
  - Row 21 changes to default 0 and range 0..32.
  - Values 1..32 select the six-operator topologies through implicit ports
    on `fm-mod`.
- (b) Keep default 5 as legacy, and add an opt-in switch. Rejected:
  - Explicit `algorithm 5` would then be unreachable.
  - A new non-row header name would shift later templates' custom ids and
    their digests.

## FV2. User instruments that already declare a non-zero `algorithm`

**Decided (owner, 2026-10-10): (a).**

**Question.** A user `inst` that declares `algorithm: int = 5` and uses
`fm-mod` used to ignore it. After this change it renders topology 5.

- (a) **Chosen.** Accept this as the intended effect of explicitly
  using `algorithm`.
  - No shipped example does this.
  - The `design-music.md` `epiano` sketch is updated to 0.
- (b) Treat only the `fm` template as algorithm-aware. Rejected: a kernel
  cannot tell which template it belongs to without a graph change.

## FV3. Excluded engine features

**Decided (owner, 2026-10-10): the exclusions stand.**

**Question.** Should the LFO, pitch EG, AMS/PMS and oscillator key sync be
added?

- **Chosen.** Not in this run. Those parameters are validated and
  ignored, as listed in the design's "Intentional simplifications". This
  keeps the engine and the tests proportionate to the request.

## FV4. Hurdy-gurdy drones in a voice-per-note engine

**Decided (owner, 2026-10-10): (a).**

**Question.** Each voice carries all four strings, so drones restart with
every melody note.

- (a) **Chosen.** Use the documented two-pattern recipe:
  - a drone pattern with `gurdy-melody 0` and a long `gate-length`;
  - a melody pattern with the drone levels at 0 and `cut 1`.
- (b) A cross-voice persistent drone. Rejected for now: it needs voice-pool
  changes.

## FV5. Single-trigger organ percussion

**Decided (owner, 2026-10-10): (a).**

**Question.** A voice cannot see other held notes.

- (a) **Chosen.** Use the per-event `organ-perc-trigger` control
  (default 1). Patterns set it to 0 on legato notes.
- (b) Engine-side detection of held notes. Rejected: it needs cross-voice
  state.

## FV6. `fm6-sysex` in the browser host

**Decided (owner, 2026-10-10): accepted as recommended.**

**Question.** Byte-file loading exists only on the native host. The browser
loader reports `host-unavailable`.

- **Chosen.** Accept this for this run. In the browser, a patch can
  still be pasted as a 155-value list. Browser file loading is a later
  editor feature.

## FV7. No prelude template for `fm6-core`

**Decided (owner, 2026-10-10): accepted as recommended.**

**Question.** Should `fm6-core` get a prelude template?

- **Chosen.** No.
  - A patch payload is per instrument, so users write
    `inst name: fm6-core freq patch: ...`.
  - The patch-less engine is reachable as `s :fm > algorithm N`.
  - This saves a template slot and three digest lines.
