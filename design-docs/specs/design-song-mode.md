# Song mode: reusable finite multi-track parts

**Status**: Accepted design — implementation in progress
**Created**: 2026-09-30

Functions can already return patterns or immutable data and call other functions. Song mode adds explicit finite multi-track parts so those functions can return editable musical structures, transform copies, repeat complete parts a finite number of times, and sequence them into one automatically terminating song. The engine continues querying symbolic music lazily; complete-song duration comes from the arrangement, not a manually supplied cycle count.

## Baseline and capability audit

Preserve design-music.md sections 1–3: sound-first chains, lazy infinite patterns, subject-first transforms, list steps, named slots and declarative time. Preserve architecture.md: shared native/browser runtime, exact rational musical time and no sleeping musical functions. Preserve lang-reference.md: tab-indented fn bodies return their last expression; calls use prefix syntax, brace groups, whitespace-separated lists, named arguments and explicit lambdas.

src/value/value.rs already represents immutable lists, dictionaries, closures, patterns, sounds and instruments. Ordinary functions therefore do not need a new calling convention to return reusable parts or invoke generators. Missing: a finite Part/Song value, checker types and corresponding native operations.

src/pattern/pat.rs explicitly describes Pat as a lazy infinite function. Stack combines children simultaneously. In structure.rs, cat_piece selects child c mod n and advances that child's local cycle using c div n; it keeps producing cycles indefinitely. Fastcat compresses child cycles into each output cycle. None provides an arrangement duration or terminal transport state.

The public repeat registered in src/vm/natives/list.rs returns a finite list of cloned values, metered by VM fuel. Internal PatNode::Repeat and step.rs lay_out support replicated pattern steps. src/vm/natives/pattern.rs does not register a finite whole-part repeat. The new API must not claim that existing repeat already means finite song repetition.

control.rs constructs new pattern nodes and samples controls at event anchors, preserving or introducing structure according to the sound-first rule. Existing transforms provide reusable building blocks. Missing: named multi-track replacement, sound-scoped selection, event-handle deletion and timeline-region replacement. region.rs edits sample playback regions; it does not overwrite an arrangement region.

query.rs uses exact half-open TimeSpan values, whole versus query-clipped part spans, and onset detection. occ.rs carries a structural path, anchor and cycle, with branch ordinals distinguishing simultaneous stack siblings. Existing identity is useful for scheduler deduplication, but includes compact NodeId hashes and lacks a user-visible snapshot/revision and chord-tone handle contract.

random.rs uses seed, node identity and cycle or anchor, so random decisions are not intentionally based on query-window order. However step.rs resolves some dynamic values at the clipped piece start, and generic pattern purity is relative to the query context. Song partition invariance must be established under frozen inputs and canonical realization, rather than assumed for every existing dynamic pattern.

src/dsp/bus.rs applies room to the sum of a bus and maintains mutable effect state per bus generation. engine/render.rs sends delay input to shared orbit state and returns orbit output into master. Applying room or delay only to bd events can still affect or mix with siblings sharing that state. A song instrument-effect branch must own its state.

src/sched/slots.rs has a slot-level muted flag and commit.rs skips muted slot events. This does not expose instrument-level hard mute within a mixed track or erase shared effect tails. Existing stop/hush operations are slot revocation, not reversible song instrument mute.

src/cli/run.rs evaluates a document and runs indefinitely or for --cycles. examples/render_track.rs provides native headless WAV export but requires cycles or seconds, snapshots one tempo, rejects tempo changes and uses an explicit tail duration. glass-teeth.vact demonstrates current manual cycle arrangements and gain patterns. Reuse these production paths; add song-derived endpoints.

src/session/protocol.rs already accepts whole-document eval with an optional span. session/eval.rs evaluates, drains and publishes each form independently. Whole-document submission therefore exists, but atomic replacement of all song definitions and routing is a separate missing behavior. host/wasm/session_half.rs carries the session protocol to the worklet; WasmRenderHost in session_hosts.rs concerns visual rendering and is not an offline audio exporter.

## Finite values

Part contains a positive exact Ratio64 duration in cycles, a dictionary of unique keyword track keys, symbolic pattern/edit nodes and instrument-route metadata. Patterns are not rendered into PCM when a Part is created. Empty track dictionaries are permitted for explicitly timed silence.

Part nodes are finite capture, sequence, repetition and immutable edit nodes. Store repetitions symbolically as child plus integer count, rather than expanding N copies. Constructors calculate duration with checked rational addition/multiplication.

part-repeat accepts an integer count from zero through u32::MAX, subject to the total-duration and work limits. Zero returns a zero-duration empty arrangement. sequence accepts a finite list of Part values; its duration is their sum. An empty sequence is valid empty music. A Song may therefore end immediately and produce only its explicitly requested tail.

Each sequence child begins at the sum of preceding durations. Each repetition starts its child at local zero. There is no implicit repeat, padding to a cycle, stretch to match another track, or automatic loop back at song end.

At capture boundaries, admit only events whose whole onset lies in [0,duration); preserve normal note and sample release behavior. A sound triggered before the end may release after it. A negative-onset continuation is not retriggered at zero.

Song wraps the finite arrangement with frozen BPM, beats per cycle, meter metadata, root seed, sample/resource references and a finite tail cap. The song descriptor and its declared arrangement endpoint are the shared transport authority.

## Proposed language surface

Every name below is proposed, not an existing native. Use ordinary functions and existing parser syntax; introduce no new grammar or string music notation.

Proposed declarations: part tracks duration: ratio -> part; part-repeat part count seed-mode: keyword -> part; sequence [part] -> part; replace-track part keyword pattern -> part; transform-instrument part keyword sound-selector (fn ctl -> ctl) -> part; part-events part keyword ratio ratio -> [event-descriptor]; delete-event part event-handle -> part; overwrite-region part keyword ratio ratio pattern -> part; instrument-fx part keyword sound-selector keyword -> part; song part bpm: ratio cycle-beats: ratio meter: [int] seed: int tail-seconds: ratio -> song; play-song song -> song.

part forces duration and the track dictionary at construction. Function/thunk values supplied inside tracks are resolved as patterns using the current language conventions. Part transforms return new values; they never bind output slots. Only play-song stages song playback.

replace-track requires an existing track key. part-events returns read-only dictionary descriptors including an opaque :handle, local whole span, resolved instrument and individual note value. delete-event accepts the handle, not a bare time position or mutable vector index.

transform-instrument invokes its callback once on the selected symbolic stream in a pure construction context; existing pattern operators may build the result. Matching occurs on the frozen source instrument before transformation. Unmatched music survives. The selected result retains track duration and is clipped by the Part boundary.

instrument-fx associates a declared bus-chain template with a selected instrument branch. The template's effect chain is instantiated privately for that branch; this is not a request to route sibling audio through the shared bus of the same name.

```vact
fn drums:
	part [drums: {s [:bd :sd :bd :sd]} hats: {s :hh > euclid 7 8}] duration: 4

fn developed:
	let base {drums & []}
	base > transform-instrument :drums :bd {p -> lpf p 900}

let intro {drums & []}
let verse {developed & []}
let arrangement sequence [{part-repeat intro 2} {part-repeat verse 4}]
song arrangement bpm: 120 cycle-beats: 4 meter: [4 4] seed: 42 tail-seconds: 8 > play-song
```

### exampleNotes

- The example uses tab indentation, existing brace grouping, dictionary pairs, explicit lambdas and subject-first pipes. A bare generator name on the right side of let binds its function value; `{drums & []}` explicitly invokes the zero-argument generator using an empty argument splat to obtain its Part. Braces alone form a block and do not call a global function name.
- Intro duration is four cycles; two intro repeats plus four verse repeats produce 24 cycles without a CLI cycle argument.
- A nested function may enumerate handles on its current Part revision, delete one selected handle, replace a track or overwrite a region before returning its transformed Part.
- No claim is made that this proposed example currently evaluates.

## Timing and randomness

Part-local time is exact rational cycles beginning at zero. Arrangement time is the sum of child offsets plus local time. Transport host time begins at one acknowledged song-start boundary. Query spans intersect the arrangement and child bounds before being mapped into local time.

For the initial release, Song owns one constant positive rational BPM and beats-per-cycle value, defaulting to 120 and 4. Meter defaults to [4 4]; numerator must be positive and denominator a positive power of two. Meter describes bars in quarter-note beats and does not silently change pattern cycle length.

Tempo and meter changes inside a Song are unsupported in this release and produce explicit diagnostics. Existing live use-bpm/use-cycle remain valid outside song playback. Reject their application to an active song rather than silently changing its duration or export result. MIDI/Link-following clocks and live input sources are likewise unavailable to deterministic song generation.

Cycle-to-seconds conversion uses the frozen Tempo contract. Host frame endpoints are obtained from absolute positions by one documented round-to-nearest, ties-up rule; never accumulate independently rounded child durations. Logical event times remain rational until the host boundary.

Default repeat seed-mode is :same: each repeat queries the child at local zero with the same derived seed and reproduces its random musical decisions. :vary derives a stable seed from the Song root seed and complete sequence/repeat placement path. Distinct placement identities remain distinct even when draws are identical. For nested repeats, :same reproduces the entire child, including corresponding draws of inner :vary repeats. The seed path normalizes each :same repeat iteration ordinal to zero, while retaining every sequence edge and :vary ordinal. The event placement path always retains the original full ordinals. Thus an outer :vary changes the child seed, while inner :same iterations share that seed; an outer :same repeats the complete inner development unchanged.

Within an immutable Part revision, edits do not reseed unaffected source streams. Seed identity and edit revision identity are separate. Native and browser use the same integer seed derivation; host timing, query size and audio callback size never enter it.

## Identity and query invariance

A Part event handle contains its exact Part revision and local logical occurrence identity. A playing Song event ID additionally contains the Song snapshot epoch and full sequence/repeat placement path. Handles from another revision are rejected, not guessed against nearby events.

Logical occurrence identity includes the complete producer path, cycle/onset identity, structure-generating branch and ordinal, and a chord-tone ordinal. Equality compares the complete identity; compact hashes are lookup accelerators only. Source byte spans and returned-query vector indices are not user-edit identities.

Expand a chord into individually addressable logical note events before song edits and host commit. Repeated identical pitches remain distinct by tone ordinal. Commit must not expand those mono events a second time. Deleting one tone leaves all simultaneous siblings intact.

Song realization uses canonical whole-cycle queries under an immutable namespace/input view. Query-dependent dynamic reads are evaluated in that canonical window, then results are clipped to the caller's requested span. Bounds at partial capture/sequence edges are applied after canonical source realization.

Canonical event identities are assigned before caller clipping and before deletion or regional selectors. Instrument branch matching and note expansion also happen before those selectors. Structural ordinals come from the producer path, not from a filtered output count.

For a fixed snapshot and successful queries, normalized union of queries over any partition equals one query over their union in event identity, whole span, controls, instrument and seed decisions. Only clipped part spans differ. Continuations retain identity and never become new onsets.

Canonical-window caching is bounded and optional for correctness: cache eviction and recomputation must produce identical results. Limits apply to canonical windows and bounded construction, so changing caller partition size cannot change admitted music. Any canonical-window failure fails song generation explicitly; it is not successful partial music.

IDs are stable within a revision/snapshot. No promise is made that byte edits, new snapshots or arbitrary reconstruction preserve event handles across revisions.

## Editing contract

All edits are immutable symbolic nodes retaining their source Part. The input and any other function caller holding it remain unchanged. Sequential edits compose in call order; each result has a new revision.

replace-track replaces only the named track's music, using the existing Part duration and local-zero origin. Other track nodes and route metadata survive. Unknown track keys are errors.

transform-instrument selects by track key and frozen sound identity. A keyword such as :bd resolves through the snapshot kit to the sound or sample-bank family it names. Matching uses resolved identity, not an effect-control name or an assumption that :bd is the track name. A selector matches all bank members of that frozen family.

delete-event checks revision, complete occurrence identity and chord-tone ordinal, and deletes precisely that logical event. An unknown or stale handle is a diagnostic. To perform another deletion on the new revision, obtain handles from that revision.

overwrite-region uses an explicit local [begin,end) range with 0 <= begin < end <= duration. It removes whole logical events on the selected track whose whole onset lies in that range and inserts replacement pattern onsets queried over [0,end-begin), shifted by begin.

Regional overwrite is onset-based: a note beginning before begin is retained, including its continuation; an event beginning exactly at end is retained; an event beginning inside the region is removed entirely. Replacement notes may release across end, but no replacement onset is admitted at end. This does not perform a destructive waveform splice.

Instrument filtering or effects affect only selected source events. Existing transforms that introduce additional selected-stream structure derive child occurrence paths; they cannot collapse simultaneous sibling identities. Song boundary clipping still applies after time transforms.

## Routing and tails

Resolve instruments and construct routes on the control side. For song playback, each track/instrument effect configuration owns a private branch containing its voices, voice effects, room/delay state and final branch gain gate. Branch output then reaches its declared track bus and master.

Existing declared bus-chain templates may supply private branch effects. The restricted route is instrument branch -> declared track bus -> master; no arbitrary user routing graph is introduced. Implement the required intermediate summing stage instead of treating today's bus-to-master graph as already capable of nesting.

For bd-only processing, send only the selected frozen bd family to its branch. Snare/hat siblings have separate branches. Translate song room/delay controls to branch-owned state rather than shared orbit/master sends. Validate that all selected effects and sidechain selectors resolve; never fall back silently to master.

Track buses and master effects are explicitly shared. A master compressor may react to bd changes and consequently alter the mixed output. Isolation promises that unselected audio does not enter bd's private effect state; it does not promise an unchanged final mix after a shared nonlinear master effect.

At a part boundary, a changed effect configuration starts a new prepared branch generation. Old voices stop receiving new onsets and their branch state drains privately. Identical consecutive configurations may retain the branch state. Never transfer an old delay/reverb buffer into a different instrument branch.

The default finite tail cap is eight seconds, configurable as a nonnegative rational Song setting and recorded in export metadata. Arrangement end stops all future song onsets and releases held voices. Render exactly through end plus tail cap; do not wait for an inferred silence threshold.

Branch generations are retained through their bounded tail deadline even after voice users reach zero, then reset and retire with acknowledgment. Apply a final bounded 64-frame fade ending at the tail deadline when a nonzero tail is requested; for a shorter tail use its available frames. A zero cap explicitly permits immediate truncation.

Reserve overlapping generations within actual host capacities before starting or applying a snapshot. If the declared arrangement cannot fit its required bounded route generations and DSP memory, reject it with the responsible track, instrument and placement.

## Live controls and snapshot application

Song data remains immutable. Live mute is a runtime overlay keyed by active snapshot epoch and frozen instrument-family identity, across all matching tracks. Provide a separate track mute only if later requested; it is not needed for this scope.

Mute preserves transport position and consumes skipped occurrences without replaying them later. It blocks future matching onsets and closes every matching branch's post-effect gain gate, including audible private tails. Use a fixed 64-frame gain ramp; hard silence is reached within 64 frames after acknowledged application.

Unmute opens the gate for future onsets; it does not resume previously suppressed notes. Reset private state and cancel matching queued voice work at mute application so stale tails cannot reappear. Other branches and transport continue.

Native host and browser worklet receive the same generation-stamped mute operation. The acknowledgment reports the actual application frame. A stale snapshot epoch is rejected. Precommitted events must be canceled or checked by the audio-side mute gate; scheduler suppression alone is insufficient.

Interactive mute is an immediate live control with a requested earliest frame.
If asynchronous delivery has already passed that frame, apply it at the next
available callback frame and acknowledge the actual application frame. Preserve
the exact requested command identity and normal family/epoch validation. This
late-control rule does not change exact activation or arrangement event timing.

Whole-code Apply is a new explicit song operation separate from existing incremental eval. Evaluate all submitted code in an isolated candidate namespace with staged effects. Freeze transitive function/global/kit/sample dependencies; do not retain mutable references into the active namespace.

Candidate code may declare functions, instruments, bus/master chains and the Song entry point. Reject unrelated slot binds, once/at, external output or capture side effects in this song transaction. Resolve all samples and prepare host resources before activation. Validation or preparation failure leaves the active namespace, transport and graphs intact.

Once all required host resources are acknowledged ready, choose the next integer cycle boundary beyond the scheduling commit horizon. Carry one snapshot epoch and activation frame through scheduling, host graph activation and publication. Cancel old queued onsets at or after that frame.

Apply restarts the new song at local zero. It does not preserve an ambiguous cursor across changed durations. Fade out the old snapshot over 64 frames before the selected boundary, clear its voices and private/shared song effect state at the boundary, and fade in the new snapshot over 64 frames. Do not mix old master tails into the new snapshot.

Report candidate-ready separately from applied; applied is emitted only after host acknowledgment. A failed preparation cancels the pending candidate. After activation begins, a host failure produces an explicit failed transport state rather than a false applied acknowledgment.

Mute overlays survive a successful Apply only for instrument-family selectors still present, mapped to the new snapshot; removed selectors disappear. Static export ignores live overlays and renders the submitted immutable Song.

Browser editor editing updates the document without changing playback. Its explicit whole-code Apply submits one revision; revision changes while a candidate is pending invalidate that candidate. Existing live incremental evaluation remains available outside this song application path.

## Playback and export

Introduce one song transport with Prepared, Playing, Draining, Ended and Failed states. Start only after required graph/sample acknowledgments. Query and commit only song onsets strictly before the declared end; schedule the end/release boundary ahead of the commit horizon.

For a document whose entry point stages play-song, vactr run without --cycles starts once and exits after the song's end plus tail cap. A legacy slot document retains its existing indefinite behavior. Reject mixing song playback and independently running legacy slots in this initial mode.

--cycles is retained for legacy runs. Reject it when a finite Song entry point is present rather than silently replacing the Song duration. No manual cycle count is necessary for complete-song playback.

Propose vactr render SCORE OUTPUT --sample-rate N for native headless WAV export. Reuse native headless audio, production scheduler, sample loader, graph acknowledgment and WAV writing already demonstrated by examples/render_track.rs. The duration comes from Song; the existing example remains a legacy exporter.

Export starts at musical zero after graph readiness, with no musical preroll. Stream bounded output blocks through the same transport and DSP graph; do not allocate a whole-song PCM buffer. Reject RIFF/frame-count overflow before opening the final output.

Write a temporary output beside the target and finalize only after successful rendering and complete diagnostics. Errors must not present an incomplete WAV as a successful complete-song export. Preserve existing source/output alias protection.

Metadata records snapshot/source revision, seed policy, BPM, beats per cycle, meter, rational duration, arrangement frames, tail frames, rate, warnings and final transport status. Explicit silence is a valid song and export; do not inherit the example exporter's rejection of all-silent PCM.

Native live playback and browser Session/AudioWorklet playback share arrangement timing, IDs, seed derivation, mute and snapshot acknowledgments. The browser uses its existing arena/ring transports and actual capability limits.

Initial browser scope is finite playback, mute and whole-code Apply. Browser offline audio export returns an explicit capability diagnostic, consistent with the existing native-tier offline-render boundary. Native export fulfills automatic static complete-song generation.

### Closed graph assets from retained construction outcomes

Before closing a candidate asset backend, execute each admitted symbolic pattern construction callback once in the existing pure query context, under the same cumulative work budget. Retain its actual callback, symbolic input and returned pattern (or addressed rejection). Walk these retained outputs to discover supported instrument families and their genuine issued fixed graph resource banks. A family computed from retained strings or lists is supported through its actual result; static keyword inspection alone is insufficient. Unused registry instruments are not blanket-pinned.

Preserve the existing event asset closure, permitted-native, no-output/no-I/O and issued Source/buffer identity rules. Generated fixed graph banks join the admitted asset union before complete bank pinning and backend disposal. This does not authorize arbitrary late callback event paths or unknown buffers. Freeze the Song and retained callback/input/output values through one shared copy cache, release its immutable asset borrow, then validate retained copied outcomes against closed assets. Re-key prepared outcomes from those actual copied values, never from stale raw addresses, and never invoke the callback again or replay randomness. Every discovery, repeated inspection, copied descriptor and stored result is admitted before growth; declaration, shape, inventory, cell and fixed-resource capture share one remaining budget. Candidate failure disposes all private state and leaves active playback unchanged.

## Bounds and diagnostics

Reuse checked Ratio64 arithmetic, VM fuel and existing pattern query bounds. Current query constants are depth 256, re-entry 64 and work budget 1,000,000; song canonical realization must honor them instead of bypassing them.

Add a bounded song construction context for recursive function-generated parts and symbolic edit depth. Its limits are fixed in the runtime configuration and advertised with host capabilities; sequence and repeat durations are checked before scheduling. A repeat count fitting u32 does not exempt its result from duration/frame limits.

Preflight tracks, route branches, simultaneously live/retiring generations, cells, samples, graph units and DSP memory against actual native/browser allocations. Do not assume a nominal voice limit also guarantees branch or arena capacity.

Keep per-query output and canonical caches bounded. Work/fuel failures, overflow, missing assets, stale handles, unknown tracks/instruments/buses, unsupported live input, tempo changes, preparation timeout and host rejection identify snapshot, source span and relevant local/global placement.

A complete-song export fails on any music realization or host fault. Live song failure stops new song onsets and publishes Failed while retaining the editor session for correction. Existing generic pattern sibling-fault behavior remains unchanged outside song mode.

Audio callbacks allocate no song data and execute no user VM callbacks. Candidate evaluation, symbolic edits, resource preparation and canonical realization run on the existing control/session side; audio receives bounded prepared commands.

## Implementation review: 2026-10-03

The immutable Part/Song model, isolated candidate evaluation, exact frame timing
and retained resource ownership remain a sound foundation. Focused checks have
shown finite Native playback, complete CLI WAV rendering and acknowledged mute.
Five real separate session/worklet WASM fixtures now pass, including finite
playback, frozen PCM, mute/unmute and actual production capacity. These are scoped checkpoints, not proof of
complete song mode or live replacement.

The review identifies two implementation blockers and one resource design risk:

- **Atomic replacement is incomplete:** the wire contract, prepared-owner
  handoff and initial DSP transaction now exist. Four Native/Arena checks pass,
  with the later pressure/lifecycle checkpoint now passing. Controller integration
  remains. Validate
  both exact owners before arming at boundary minus 64 frames. Internal new
  resource adoption may occur silently at arm; new events and audible output
  remain gated until the exact selected boundary. At that boundary clear old
  state and report genuine new Applied. Rejection before arming preserves old
  audio. No requester success may be inferred from internal adoption.
- **Mute continuity and cancellation remain integration work:** initialize the
  new certified mute overlay before its first onset. Remap by frozen instrument
  definitions and asset provenance, rather than numeric instrument/file IDs.
  Serialize pending mute requests before snapshotting. Distinguish cancellation
  before arming from cancellation after the old fade has already begun.
- **The original capacity estimate was wrong:** actual preparation of unchanged
  generated-parts.vact requires 25 bus slots, 22 pools and 47 resources across
  11 branches. Conservative source-family coverage and overlapping generations
  contribute to that demand. The previous six-bus/thirteen-slot estimate is
  superseded. Two such owners plus the legacy master require 51 physical buses.
  Raising the current six-slot profile to 51 adds about 349.92 MB Native state
  or 155.736 MB browser state at 48 kHz, before output buffers and metadata.
  That uniform-allocation proposal is superseded by the verified heterogeneous
  layout: six original full regions plus 45 smaller constructor-owned regions.
  Added state is about 82.8864 MB on either tier at 48 kHz, excluding scratch
  and metadata. Production adapters now select that layout and 128 templates.
  Five adapter tests pass, including genuine overlapping original preparations,
  one-below refusal preserving old audio, cleanup and compatibility defaults.
  Template sizing includes the real 73-template prelude and both retained owners.

The follow-up probe uses the genuine Session bootstrap and unchanged score.
At 8 kHz, an explicitly configured measurement host has 55 free templates after
73 prelude templates install; song preparation consumes 22 templates and
25 buses. Minimum required bus state is 1,527,976 float frames, while the current
uniform allocation removes 8,600,000 frames from available physical capacity.
The probe and three handoff tests pass; two opaque ownership compile-fail docs
also pass. These results justify changing the physical allocation layout, not
reducing the certified routes. The initial DSP replacement checkpoint is implemented; its pressure and
controller acceptance remain incomplete.

The atomic DSP checkpoint now passes nine genuine Native/Arena fixtures:
exact replacement with primed mute, pre-arm cancellation, after-arm failure
ownership, replacement after natural old retirement, full actual ACK pressure
with a later control marker, failed physical validation preserving old PCM,
callback partition invariance, old deadlines around F/A, and real returned-slot
reuse without resetting new audio. Eight host handoff/recovery fixtures also
pass, including cancellation followed by a successful second Apply. The exact
private endpoint reservation test and four transport regressions pass. These
results cover their named invariants; broader build/lint and controller
acceptance are still pending. The fresh production-profile WASM
artifact passes five playback/mute/profile ABI tests. Live replacement through
Session still fails its exact cycle-boundary and mute-continuity checks, so the
controller integration remains mandatory. No overall song-mode completion is
claimed from the DSP or capacity checkpoints.

### Follow-up corrections from the implementation review

Cancellation must preserve retry authority as well as audible playback. The old
transport currently issues an opaque replacement source only once. A candidate
that never activates must return that same source after authenticated cancellation,
correlated activation rejection when submitted, and all original resource returns.
The matching old transport may then reclaim it and permit a subsequent Apply.
Armed or Applied candidates cannot return never-activated authority. Five genuine recovery fixtures now pass on Native and encoded Arena paths,
including a successful second Apply and authentic exclusions after arm. Broader
build/lint completion remains pending.

ACK pressure also requires explicit ordering and capacity ownership. A retained
cancellation failure must not be overtaken by a later control receipt or boundary
Applied. Reserve endpoint obligations before arming so other commands cannot
consume capacity needed by the boundary commit. The five new pressure/lifecycle fixtures now pass alongside the original four
DSP tests. Full ACK pressure is measured from actual reports; bounded cleanup
waits derive from retained resource-return obligations.

The controller plan is Ready after review of closed asset access. Certificates
retain original Arc PCM, normalize declaration/control sites, and compare entire
ordered banks including wrapping policy. Resource capture of bank member zero
alone cannot certify a whole bank. The controller must still satisfy the two
actual failing browser replacement checks before live Apply is accepted.

### Resource allocation and bounded admission

The current Engine gives every physical bus the same worst-case state buffer,
including granular capture state, even when its frozen effect chain needs much
less. Increasing slot count therefore multiplies that worst-case allocation.
The private bus contract additionally requires two four-second delay regions;
these cannot simply be removed while retaining existing semantics.

Reducing generation reservations from two to one is not justified by the current
proof: release tails can overlap the next placement. Nor does matching resolved
instrument identity prove that source branches share placement, effects and
ownership. Preserve both until a stronger lifetime or exclusivity proof exists.

Select heterogeneous constructor-owned slot sizes as the next bounded
implementation: retain the original six full-budget regions and provide extra
song slots whose sizes cover the measured frozen chain requirements. Both Native
and Arena adoption must select the smallest fitting free region deterministically
so neutral chains do not consume the full-budget regions unnecessarily. Preserve
conservative unadopted claim shielding and prove mixed small/large graph pressure.
Aggregate frame capacity is not a guarantee that every graph fits an individual
region; actual staging may refuse before Ready, with exact cleanup and old audio
preserved. Measure actual admission for both complete original owners.

A bounded shared state pool remains a future option, but needs reservation,
fragmentation and exact region ownership changes. Control-side graph-sized memory transfer
would additionally require explicit returned ownership in Native and Arena;
callback allocation or deallocation is prohibited. Any chosen layout must admit
both complete owners before the old fade, retain the supported effect budgets,
and refuse larger arrangements without interrupting existing audio.

Browser clock refresh and late interactive mute handling have been corrected
and verified through the actual WASM ABI. This does not establish browser UI or
physical device acceptance. Historical failing runs remain in the completed
browser-runtime evidence plan; its final four-fixture checkpoint is the current
result. New source changes require a fresh artifact and renewed verification.

Continue in dependency order: resolve the capacity/memory decision, implement
and verify the DSP replacement transaction, then integrate Session commit,
semantic mute remapping and document-edit cancellation. Verify rejected Apply
against unchanged audio, exact fade boundaries, no old-tail leakage, first-onset
mute suppression and eventual resource reuse. Record completion only after
these remaining paths are demonstrated. No rewrite of Part/Song is indicated.

## Plan-author handoff

Use one plan author to cover every phase. Persist this draft and add only a song-mode reference under architecture.md before creating durable plan references. Preserve the existing dirty design-music.md and document indexes byte-for-byte except authorized song index additions.

Every plan belongs under impl-plans/active/song-mode-*.md, contains at most 1000 lines, eight modules and ten tasks, and targets one to three sessions. Split integration adapters into additional bounded plans when their exact file count exceeds those limits; do not hide many touched modules under a single integration task.

Future native Riela waves must declare output contracts, dependencies and exclusive file ownership. Parallel implementation is permitted only after the shared contract wave is complete and only for noninterfering file sets. These are future implementation waves, not extra design or plan authors in this execution.

### dependencyWaves

- {"wave": "1", "proposedPlan": "impl-plans/active/song-mode-values.md", "dependsOn": [], "deliverables": ["src/song/mod.rs", "src/song/part.rs", "src/song/song.rs", "src/song/limits.rs", "src/value/value.rs", "src/types/ty.rs", "src/lib.rs"], "contracts": ["Part: immutable symbolic node, checked rational duration and track dictionary.", "Song: arrangement, frozen time/seed settings and finite tail cap.", "SongLimits: construction and realization capacities derived from runtime/host allocations.", "Public declarations should include fn duration(&self) -> Ratio64 and checked constructors returning Result<Part, Failure>, without implementation bodies."], "completionEvidence": "Types distinguish Part/Song from Pattern; finite duration and count rules have explicit future unit-test criteria."}
- {"wave": "2A", "proposedPlan": "impl-plans/active/song-mode-query-edits.md", "dependsOn": ["1"], "deliverables": ["src/song/query.rs", "src/song/edit.rs", "src/song/identity.rs", "src/song/tests/mod.rs", "src/song/tests/query.rs", "src/song/tests/edit.rs"], "contracts": ["fn query_part(part: &Part, span: TimeSpan, cx: &mut SongQueryCtx<'_>) -> Result<Vec<SongEvent>, Failure>;", "fn delete_event(part: &Part, handle: &EventHandle) -> Result<Part, Failure>;", "fn overwrite_region(part: &Part, track: KwId, region: TimeSpan, replacement: Rc<Pat>) -> Result<Part, Failure>;", "EventHandle, PartRevision, PlacementPath and SongEvent declarations must expose exact identity and mono-note semantics."], "completionEvidence": "Future tests cover partition equivalence, fractional boundaries, identical simultaneous events, chord-tone deletion, stale handles, seed modes and unchanged inputs."}
- {"wave": "2B", "proposedPlan": "impl-plans/active/song-mode-language.md", "dependsOn": ["1"], "deliverables": ["src/vm/natives/song.rs", "src/vm/natives/mod.rs", "src/types/natives_domain.rs", "src/types/ty.rs", "src/value/eq.rs", "src/vm/call.rs"], "contracts": ["Declare native registration and Part/Song native signatures using repository forcing masks.", "Resolve all exhaustive Value/Ty consumers found by source review, with explicit additional bounded plans if necessary.", "Type equality for opaque event handles and revisions; do not expose mutable event arrays."], "completionEvidence": "Future parser/type/VM fixtures verify proposed syntax without grammar extensions and reject unsupported Part-as-Pattern coercions.", "ownershipNote": "src/types/ty.rs overlaps Wave 1; Wave 1 must complete first, and any additional shared integration path must be assigned to one wave."}
- {"wave": "3", "proposedPlan": "impl-plans/active/song-mode-snapshots.md", "dependsOn": ["2A", "2B"], "deliverables": ["src/song/snapshot.rs", "src/session/song.rs", "src/session/mod.rs", "src/session/protocol.rs", "src/session/codec.rs", "src/session/session.rs", "src/ns/stage.rs"], "contracts": ["SongSnapshot, PreparedSong, SnapshotEpoch, ApplySongBody and SongInstrumentMuteBody declarations.", "fn prepare_song(&mut self, request: ApplySongBody) -> Result<PreparedSong, Failure>;", "Specify isolated candidate namespace ownership and dependency freezing rather than shallow-cloning VarSlotRef values."], "completionEvidence": "Future failure-injection checks demonstrate no partial namespace or audio application and stale-revision rejection."}
- {"wave": "4", "proposedPlans": ["impl-plans/active/song-mode-routing.md", "impl-plans/active/song-mode-host-commands.md"], "dependsOn": ["3"], "deliverables": ["src/song/routing.rs", "src/dsp/song.rs", "src/dsp/mod.rs", "src/dsp/bus.rs", "src/dsp/engine/render.rs", "src/dsp/ring.rs", "src/host/caps.rs", "src/host/wire.rs", "src/host/native/audio.rs", "src/host/wasm/messages.rs", "src/host/wasm/worklet_half.rs", "src/dsp/arena.rs"], "contracts": ["SongBranchId, SongRoutePlan, branch-generation tail ownership and epoch-stamped activation/mute commands.", "fn prepare_routes(snapshot: &SongSnapshot, caps: &CapabilitySet) -> Result<SongRoutePlan, Failure>;", "Declare audio command acknowledgment and transport codec fields without callback implementation bodies."], "completionEvidence": "Future DSP/host checks verify bd isolation, delay/room ownership, private-tail muting, acknowledgment consistency and bounded overlap.", "ownershipNote": "Divide these twelve paths between at least two plans with explicit shared command contracts and sequential ownership of shared codec changes."}
- {"wave": "5", "proposedPlan": "impl-plans/active/song-mode-transport.md", "dependsOn": ["4"], "deliverables": ["src/sched/song.rs", "src/sched/mod.rs", "src/sched/runtime.rs", "src/sched/commit.rs", "src/sched/slots.rs", "src/clock/clock.rs", "src/session/publish.rs"], "contracts": ["SongTransport, SongTransportState, arrangement endpoint and tail deadline declarations.", "fn start_song(&mut self, song: PreparedSong, activation_frame: u64) -> Result<(), Failure>;", "fn mute_instrument(&mut self, epoch: SnapshotEpoch, selector: InstrumentSelector, muted: bool) -> Result<(), Failure>;"], "completionEvidence": "Future tests cover end-exclusive onsets, release/drain completion, queued-event cancellation, restart-on-Apply and fractional host-frame mapping."}
- {"wave": "6A", "proposedPlan": "impl-plans/active/song-mode-native-export.md", "dependsOn": ["5"], "deliverables": ["src/cli/args.rs", "src/cli/run.rs", "src/cli/render.rs", "src/cli/mod.rs", "src/song/export.rs", "src/host/native/mod.rs", "src/song/tests/export.rs"], "contracts": ["SongExportOptions and SongExportReport declarations.", "fn export_song(song: &SongSnapshot, options: &SongExportOptions) -> Result<SongExportReport, Failure>;", "Specify a reusable WAV writer boundary, extracting from the current example only if required and listing its exact paths in an additional plan."], "completionEvidence": "Future checks cover automatic run completion, exact frame counts, silent songs, RIFF limits and absence of finalized partial output."}
- {"wave": "6B", "proposedPlans": ["impl-plans/active/song-mode-browser-session.md", "impl-plans/active/song-mode-editor-controls.md"], "dependsOn": ["5"], "deliverables": ["src/host/wasm/session_half.rs", "editor/src/protocol/types.ts", "editor/src/protocol/envelope.ts", "editor/src/protocol/client.ts", "editor/src/protocol/store.ts", "editor/src/app/apis.ts", "editor/src/code/sync.ts", "editor/src/app/song.ts", "editor/src/app/main.ts"], "contracts": ["Apply/preparation/application/failure messages and actual application-frame acknowledgments.", "TypeScript declarations for whole-code Apply, pending revision and instrument-family mute state.", "Advertise browser song-playback capability and explicit native-only offline export."], "completionEvidence": "Future browser fixtures verify revision races, worklet acknowledgment, mute latency and rejected candidate preservation.", "ownershipNote": "Split the nine paths into two plans and make editor-controls depend on the completed browser-session wire contract."}

### futureVerificationOnly

- CARGO_TERM_QUIET=true mise run fmt-check
- CARGO_TERM_QUIET=true mise run check
- CARGO_TERM_QUIET=true mise run clippy
- CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 mise run test
- CARGO_TERM_QUIET=true mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm
- The plan author must read editor/package.json and name its actual applicable test scripts; do not invent script names.
- Future song fixtures should cover nested generators, unchanged originals, finite fractional sequencing, seed policies, simultaneous siblings, chord-tone deletion, overwrite boundaries, bd-only effects/tails, whole-code rollback, native/browser timing and complete-song export.
- These commands and fixtures are proposals for future implementation verification. None ran in this node.

### repositoryImplementationRules

- Future Rust modifications must use the repository rust-coding agent and automatically invoke check-and-test-after-modify.
- Any touched Rust source file at 1000 or more lines must be split according to the Rust coding standards, with the affected exact paths and ownership recorded in its plan.
- Do not modify existing dirty source while authoring plans. Future implementation must recheck its baseline before touching those files.

## Implementation contract refinements (2026-10-01)

The accepted behavior requires full producer tracing separate from legacy OccKey
hashes. SONG-04A/B are bounded prerequisites for that trace; scheduler dedup keys
remain unchanged. SongSource is an internal selected-stream pattern node so ordinary
transforms compose with parts. Transform construction receives a pure frozen VM
context, invokes its callback once, and stores a prepared pattern. EventHandle is
opaque and issued only by successful realization; it includes track and finite
placement identity and preserves fractional notes. These corrections strengthen the
planned declarations without narrowing song-mode behavior.


### Verified review checkpoint: 2026-10-03

Original checker process68596 exited0 with host8/runtime9, no ignored or filtered
public tests. Log /tmp/vactr-pressure-recovery-checkpoint-006.log
(SHA37128892c5fda0c839e33a7f0df285514ed7c4083bea8c374dbc86dc71f38d22).
All16 held source/plan hashes still match their pressure0006/recovery0008
receipts. Private endpoint reservation test90595 and full transport test3455
also exited0 (1 and4 tests respectively). Native check, pure WASM, strict lint
and fresh browser regression continue separately. Live controller acceptance
remains red and full song-mode completion is not claimed.


Checkpoint follow-up: strict all-target lint and scoped formatting now pass after
a semantically identical range check correction in the declared controller parent.
The fresh f0057a18deeaf7a5b8beff2181be72e3a503eddf324984a23f8be209324ca0de
artifact passes all five browser playback/profile regressions (original52999exit0).
Controller integration has been released for actual implementation. The pressure
criterion audit retains three dedicated proof obligations: cached failed timed
command ordering under ACK pressure, actual Engine exact/one-short endpoint
ingress refusal, and invalid previous physical authority preserving old PCM.
Private reservation-table checks and new-lease rejection do not substitute for
those wider witnesses. They are being implemented in the existing pressure scope,
while controller code proceeds in disjoint modules. Both remain incomplete.

### Focused design and implementation reassessment: 2026-10-03

The immutable Part composition contract remains the intended architecture. The
current review concentrates on the live Apply transaction, where physical resource
ownership, queued commands, musical boundaries and publication must agree.

- Ordinary queued activation cancellation previously returned preparation resources
  without removing future commands and endpoint reservations. A production repair
  and three additional pressure witnesses are source-ready, awaiting independent
  joined verification. They are not counted as passing tests yet.
- The controller boundary calculation currently uses `ceil(R*d/n)` for cycle index,
  which can skip the earliest legal rounded frame boundary when frames per cycle
  are fractional. For positive integer relative minimum R, the earliest index is
  `floor((R-1)*d/n)+1`; R=0 permits index zero. Checked arithmetic and a regression
  must verify this before accepting the controller.
- Three actual browser Apply acceptance fixtures remain red against the last
  verified WASM artifact: old cycle alignment, acknowledged carried mute publication,
  and compound replacement cancellation/retry. The latest run exited 1; its log is
  `/tmp/vactr-song-apply-current-baseline-001.log`. Retry requests now correctly carry
  the edited document's edit epoch. No new controller artifact has been accepted.

The next checkpoint requires held controller and pressure sources, independent
native/runtime checks, a newly built WASM artifact, and these same browser fixtures.
Full song mode remains in progress. This review does not authorize scope expansion
or claim successful controller integration.

Reassessment execution checkpoint: initial native controller compilation passed.
After removing its unused import, 33 independently executed focused tests passed:
runtime 12, handoff 8, transport 4, boundary 2, clock 6 and endpoint reservation 1.
The fresh WASM artifact has SHA256
`60c9e21240f59a813972e26029d6a95ffbec44adcd8c44d780a5f46394bef0a3`.
Its browser acceptance revealed one production publication defect and two fixture
ABI errors. Carried mute publication used requester connection zero, which the
browser drops; the author has written a telemetry broadcast correction awaiting
verification. LeaseReturned encodes resource id/generation before kind, and the
worklet outputs planar stereo rather than interleaved samples. Correcting those
fixture interpretations preserves all original cleanup, spectral amplitude and
old-tone rejection assertions. The full eight-test run then passed seven tests,
with carried mute publication the sole failure against the earlier artifact
(`/tmp/vactr-song-controller-browser-layout-004.log`, actual session45083 exit1).
The dedicated Native/Arena controller suite is still being authored. This is a
verified partial checkpoint, not completion of the controller or song-mode goal.

The carried mute correction and first two genuine Session controller fixtures
now pass independent verification through both Native and Arena paths. Native
check and pure WASM build also pass. Artifact SHA256
`1d0cfc4ced4b9c23ae473dc526f74e614352b0a9be307c7cb01123418000ace2`
passes all eight browser fixtures (actual session99569 exit0,
`/tmp/vactr-song-controller-store-alias-browser-006.log`). Replacement messages
are decoded through the actual protocol decoder and applied to the editor Store;
the store retains both carried builtin aliases and numeric instrument selectors
at the authentic application frame. Full controller criteria for Draining,
pressure, definition changes and document races remain unfinished.

Sample admission reassessment confirms another required correction. The current
owner turns every Arc-only sample refusal into a retry, including permanent
invalid geometry, sender metadata exhaustion and byte exhaustion. The existing
WASM sender observation helper has no caller and does not include its actual
retained metadata ceiling. Generic Rejected also clears sender reservations
before authoritative LeaseReturned. The existing eight-path sample-admission
plan is now released for typed refusal, independently measured sender limits,
complete union admission and genuine cleanup. Its sources are disjoint from
the controller scope. No guessed quota, reduced arrangement or synthetic host
outcome substitutes for this correction; implementation and verification are
still pending.

The joined typed-sample/controller checkpoint now passes seven genuine controller
tests (actual process16572 exit0) and two initial sample tests (foreground7eee17
exit0). Controller witnesses include Draining/natural old End before replacement,
different new tempo on the old boundary, changed/renamed/removed definitions,
changed unused bank PCM, pre-arm retry, after-arm catalog preservation, and real
command pressure. External pressure receipts have their own explicit fixture
consumer; Session-owned outcomes are forwarded unchanged. The initial sample
tests prove permanent geometry and unavailable refusals retain the exact Arc;
complete union boundaries and actual queue/cleanup pressure remain unverified.
Separate bounded followups declare genuine controller ACK saturation and three
existing fixture adapters' checked sample forwarding. Their code/checks remain
required, along with the full sender admission fixtures and broad final gates.

Timing reconciliation: the earlier controller refinement described ceiling-rounded
cycle boundaries, but the original Timing and randomness contract and actual
SongLimits::frames_at use nearest-frame rounding with ties up. Non-half fractional
cycle positions expose a one-frame mismatch. Live replacement must use that same
absolute conversion, without accumulating child durations. For relative minimum
R>0 and exact frames-per-cycle n/d, select the earliest integer k satisfying
`2*k*n >= (2*R-1)*d`; R=0 permits k=0. Checked quotient/remainder arithmetic and
regressions against the actual shared converter supersede the earlier ceiling
formula. This correction preserves the original timing contract; it does not
change the event/export converter or introduce a second rounding policy.

### Focused reassessment: 2026-10-03

The current evidence supports continuing with the existing finite Part/Song and
ACK-authoritative transport architecture, subject to the remaining acceptance
gates. It does not establish full song-mode readiness. The real Native/Arena
controller suite now passes eight tests including physical ACK pressure
(`/tmp/vactr-controller-sample-checkpoint-002.log`, process79547); that joined
process still exits101 because two sample fixtures fail. The corrected shared
nearest/ties-up replacement boundary passes all three private regressions
(`/tmp/vactr-controller-rounded-private-001.log`, process76830 exit0).

The current sample failure precedes sender admission. Sixteen distinct sample
families require sixteen private generations and eighteen free buses (private
branches, one track and the song master). The fixture allocates twelve physical
buses and retains a legacy master, leaving eleven free. Seventeen families also
need seventeen template/branch slots and nineteen free buses. The fixture must
supply these independent graph capacities while retaining the actual sender's
sixteen-resource limit. Its one-over sender refusal and cleanup assertions must
remain intact. The second failure is a mutex poisoned by the first assertion;
it provides no independent evidence about the admission race.

Finish the bounded admission correction, then run the complete library/song
integration scope, strict native/WASM checks and a freshly built browser
artifact. Historical browser passes do not validate the latest source changes.
No further feature expansion or full redesign is justified by this checkpoint.

The focused controller source review also finds three unclosed failure paths.
Runtime tick restores every start_ready refusal to Ready, including permanent
work/overflow failures, rather than terminating and retiring that candidate.
Replacement priming can stop on genuine Backpressure before Replace admission,
but the owner loop still calls advance with activation_posted=false. Pending
mute submission discards the refusal class, so permanent Invalid/Unavailable
can indefinitely block replacement. Fix these transitions within the existing
controller plan and add targeted witnesses before broad acceptance. A pending
replacement must continue submission and cancellation/cleanup without realizing
music before admission; permanent failures must report once and retain actual
ownership until authoritative cleanup. Passing the existing eight controller
tests does not close these newly identified paths.

The current sample-admission six-test suite now passes (process97916 exit0),
including sixteen-resource sender admission, genuine seventeen-resource refusal
and observation-race cleanup. The held controller's four private tests also
pass (process41204 exit0), including permanent startup failure retirement.
Public partial-priming and permanent-mute regressions are still being authored.

The original-scope audit confirms missing live command isolation: incremental
tempo/clock changes and legacy slot/one-shot scheduling remain accepted after
finite Song activation. Starting a Song checks existing legacy slots, but the
reverse direction is unguarded. The bounded live-isolation plan supplies a
read-only actual-resource ownership guard and diagnostics before these live
mutations; historical Ended/Failed alone must not permanently block legacy use.
Heterogeneous/source-trimmed Index geometry and retained dynamic occupancy also
remain unclosed in their existing plans. They are not claimed complete by the
passing controller, sample, export or consumer fixtures.

### Review decision and verified acceptance: 2026-10-03

Retain the immutable Part/Song composition and acknowledgement-driven transport
architecture. The current evidence supports this decision: both serial end-to-end
tests and the live-isolation test pass (process30800 exit0). They exercise nested
generation, selective chord editing, unchanged repeated siblings, exact native
and portable-WASM command/PCM agreement, finite completion and resource cleanup.
Independent snapshots issue distinct Part revisions; their comparison uses an
authenticated revision correspondence, while partition checks within each
snapshot retain exact original identities.

The canonical example also completes an actual native run automatically and
exports 2,688,000 stereo PCM frames at48kHz:48 seconds of arrangement plus an
eight-second tail. Fresh native checking and a pure-WASM build pass;33 selected
browser/editor tests pass against that fresh artifact, including mounted Apply
and mute controls connected to the actual backend.

Full readiness is still unproved. Close the remaining lifecycle and retirement
regression assertions against actual receipts, then rerun the full regression
scope. Heterogeneous/source-trimmed Index routing and retained dynamic occupancy
remain implementation gaps. The static-joint plan is Planning: its proposed
integer elimination algorithm lacks a proved storage/work bound and must not
be treated as an implementation-ready deliverable.

The next design review should resolve that bounded algorithm and its shared
admission/query contract before authorizing its Rust changes. Keep all original
requirements visible in the serial acceptance ledger; additional narrow plans
must lead to working acceptance evidence. Full redesign is not warranted by
the passing composition/playback path, but declaring the current system
problem-free or complete would be inaccurate.

Bounded source review confirms the next implementable unit is a shared,
authenticated static-family program, extracting the existing trace/copy replay
and preserving exact owner/source emission constraints. Keep the proved cluster
fast path while developing genuine heterogeneous recognition and quota fixtures.
This unit prepares the general solver; it does not complete heterogeneous
geometry or dynamic occupancy. A general solver still needs an explicit
quantifier/disjunction representation and a proved congruence/storage bound;
the statement that integer elimination exists is insufficient.

The retirement fixture now passes all five host-acknowledgement tests. The
lifecycle diagnostic observes the original epoch99/frame450 activation rejected
as Malformed after it becomes late; its old NotReady expectation is stale.
Preserve the original pressure scenario and correlation when correcting it.

### Remaining occupancy implementation direction

The original canonical-query contract permits bounded canonical realization;
it does not mandate a general integer-elimination solver. Preserve existing
symbolic fast paths and investigate one retained occupancy authority for
unresolved static and dynamic Index operands. This changes the implementation
approach, not the required supported geometry, identities or admission safety.
The static-joint solver's stronger algorithm restrictions are not additional
user requirements.

Realization must use the original isolated snapshot evaluator and observe Index
wholes before subject sampling or note filtering. Retain complete operand and
producer authority; source-START eligibility precedes connected-component
merging, and caller clipping follows it. Admission and configuration must
consume that same retained authority, under charged work and storage limits.
Never use emitted-note counts as a configuration-opening proof.

Complete bounded realization can certify one original owner. It cannot alone
certify arbitrary varying repeat seeds. Arrangement repeats must remain
symbolic; their admission needs a proved and enforced uniform opening ceiling,
not a scan of all placements or replay of one observed seed. Mutable snapshot
issuance and immutable route consumption also need an explicit integration
contract. These are concrete remaining proof and implementation tasks; no
canonical fallback is claimed implemented or ready by this review.

The first executable occupancy unit must retain genuine dynamic callback
results as well as pre-subject Index observations. Otherwise preparation and
transport could execute the callback independently instead of consuming one
snapshot authority. Query operations, actual VM instruction debits, copies and
unions must share an incoming ledger. Existing per-query QState fuel and
per-call restored VM fuel do not already provide that cumulative certificate.
Immutable admission and later query replay are required consumers of the same
opaque retained result. The bounded implementation handoff is
[canonical occupancy](../../impl-plans/active/song-mode-canonical-index-occupancy.md);
its first metering/authority task is ready, while consumer and varying-seed
admission gates remain open.

### Canonical collector review: owner selection and clocks

The first collector draft invokes `observe_part` on the original root with all
tracks for each requested occupancy address, then filters captured observations.
This is not yet the addressed realization required above: unrelated sibling
callbacks can execute and consume the shared budget before filtering. A complete
callback journal scoped to the requested owner cannot imply that execution was
limited to that owner. Resolve owner selection before accepting the foundation.

Keep root arrangement windows, local owner windows and Index issuer spans
explicit. A nested owner's local window cannot be passed as a root arrangement
window without its authenticated placement mapping. Preserve original seed and
placement traversal when selecting the owner; do not query a detached child with
a newly derived root seed. Genuine candidate fixtures must include a nested
nonzero offset and an unrelated callback that would fail if executed. These
fixtures must establish correct full producer identities and callback exclusion,
in addition to the existing fractional replay and cumulative work checks.

This is a review of an unfinished draft, not a failure of the already verified
composition/playback checkpoint. Immutable admission consumers and uniform
varying-seed bounds remain required after the collector is corrected and checked.

The production mutable handoff is `SongHostPreparation::prepare` in
`host/caps/song/preparation.rs`: it owns the prepared snapshot and original
cleanup work counter before calling immutable `prepare_routes`, resource
assembly and the Reserve ledger. Retained occupancy must be completed there
before any Reserve is submitted. Keep capacity observation and actual clock
validation in their existing order. Route preparation, geometry lookup and
subsequent song queries must consume the same retained result; adding a cache
that only private fixtures read does not satisfy this integration requirement.

Current consumer signatures confirm the missing bridge: `source_density` reads
only `FrozenRoutingInventory`, and `index_configuration` receives bound Slice
operands rather than the snapshot. Neither can read private snapshot occupancy
without an explicit borrowed authority contract. The collector currently
retains Index observations and an ordered callback journal but discards complete
canonical SongEvents. Ordinary `SongSnapshot::query` still invokes the evaluator;
observation replay alone therefore does not prove callback reuse during playback.
The next bounded phases must connect both consumers and ordinary query reuse,
with full owner/window execution context rather than a callable/argument cache.

Author review found an additional cumulative-metering gap: native `part-events`
creates fresh public VmQuery adapters inside a callback. Those nested guarded
calls can restore fuel independently of the outer canonical adapter. Direct
callback fuel fixtures do not prove these transitive native paths. The
foundation remains incomplete until a bounded VM/native bridge and a genuine
nested callback sufficient/one-less fixture establish the same cumulative
ledger across those adapters. Do not substitute a documented limitation for
this required accounting invariant.

The source-grounded next phase is the seven-path
[VM/native meter bridge](../../impl-plans/completed/song-mode-canonical-native-meter.md).
It debits the original ledger at actual VM/native ticks, preserves consumed
fuel across nested guards, and shares dependent Part query work/depth without
applying the output occupancy filter to computational callback dependencies.
After that bridge is independently verified,
[ordinary query replay](../../impl-plans/completed/song-mode-canonical-query-replay.md)
connects raw original owner outputs to playback; immutable admission remains
the subsequent required handoff.

The genuine nested replay checkpoint exposes a separate prerequisite traversal:
TransformInstrument evaluates its source arrangement before filtering selected
instruments and querying the replacement pattern. Raw outer output does not
replace that earlier source execution. Retain the target's authenticated
prerequisite owners from the original edit/source ancestry, using the same
dependency selection for observation and raw execution capture. Each record
keeps its full original placement, seed, entry trace and local-cycle key.
Preserve evaluation order and unaffected instrument rows; unrelated sibling
owners remain excluded. Acceptance compares complete event/source identities
and forbids callback reads during replay, rather than prescribing one record.

The bounded replay phase is now independently accepted:348 distinct Rust tests
pass, current native/strict lint/WASM/format gates pass, and the fresh artifact
passes590 editor tests and frontend build. Explicitly retained snapshots replay
original raw owner and prerequisite executions through ordinary query without
the observed callbacks. This does not yet install retention in production host
preparation: immutable admission and the pre-Reserve handoff remain required.
See the [completed replay plan](../../impl-plans/completed/song-mode-canonical-query-replay.md)
and the next [runtime clock phase](../../impl-plans/completed/song-mode-canonical-clock-frames.md).

### Uniform dynamic occupancy: source facts and enforceable bound

Source review of step_events, Pure, squeeze and query_slice confirms that a
dynamic leaf's nominal slot does not bound every returned whole. Scalars emit
one slot event and Nil emits none; Lists can subdivide a Steps slot, and returned
Patterns contribute their actual child wholes. squeeze clips the queried part,
not the whole: a returned Slow pattern can extend beyond its nominal slot.
Existing timing capture labels Fn/Thunk as dynamic without certifying a
scalar-or-Nil result. One observed scalar result is not a uniform certificate.

An executable conservative proof direction is an opaque, authenticated finite
owner execution domain with a sealed cumulative work ceiling W. Every seed's
successful execution must debit that same ceiling, including at least one unit
for every actual pre-subject Index observation. Then each successful execution
has at most W observations, independently of its seed. For a fixed authenticated
source/use/configuration group, qualifying whole selection and connected union
cannot create more components than its qualifying observation footprints.

Do not immediately generalize this per-group result to an aggregate instrument
family: one Index row can sample several subject use identities. Aggregate
admission must prove and compose their multiplicity or debit every actual
group footprint/component in the same uniform ledger. Source-START eligibility,
whole reach, owner clipping, placement overlap and tails remain part of that
proof. Symbolic arrangement multiplication must stay checked and must not
enumerate varying placements. Large conservative bounds can truthfully fail
host fit; that is not evidence of the pattern's actual minimal demand.

The observation guard currently exists only in retained realization. Its use
in every future ordinary realization, immutable admission and replay must be
enforced before this bound can certify readiness. A smaller scalar-slot bound
requires an authenticated checked return contract and runtime enforcement,
not a guess from one return. This section records a source-grounded proof
direction; it does not claim the required uniform consumers are implemented.

### Immutable consumers and authentic clock capture

The source-grounded immutable bridge is the private FrozenSourceUseCover. It
already travels from builder certification to density and later stage.cover
resolution. A checked companion can retain the same snapshot-owned view there
without adding mandatory fields to public SongRoutePlan or mutating frozen
recipes. Top-level and nested cover certification must receive the same view.
Attaching it after existing certification has already refused unresolved Index
does not implement admission; truthful failure remains until geometry is proved.

Retained lookup methods must authenticate full owner/address/placement/seed/local
window and charge lookup/copy work. No immutable consumer may access the VM or
execute a callback on a cache miss. Distinct use identities remain separate
before exact connected union and source-START eligibility.

Current issuer_clock supports preserved/unit and static Rate/Shift layouts.
Dynamic and nonunit composed mappings need actual query-issued projection.
Capture checked frames before recursive child q: fast_by has the evaluated
factor, squeeze the actual step width/base, Iter its shift and rev_piece its
mirror/piece. map_times is too late and has no observer state when subjects
emit nothing. Preserve issuer and owner bases, queried pieces and uncut wholes;
reversal needs explicit orientation and half-open boundary handling.

Grid/Segment and other first-structure sampling reassign a sampled point to a
new structural whole. Record that relation rather than treating it as an
invertible affine map. Selected SongSource boundaries retain the authentic
source whole, issued handle and inherited clock relation. Uninstrumented
clock-changing paths must carry an Unknown barrier, never default to identity.
Temporary truthful failure at such a barrier is not the final supported scope:
the missing runtime hooks and geometry consumers remain required implementation.

### Design and implementation review: current completion path

The function/Part composition architecture still matches the requested static
song workflow. Current evidence does not justify an architectural rewrite, but
the production completion path is unfinished. Treat verified playback/replay
and unresolved admission as distinct acceptance results.

Source inspection confirms that SongHostPreparation::prepare calls
prepare_routes without first calling retain_index_occupancy. Builder source
certification and density still receive the frozen inventory rather than a
borrowed retained snapshot authority. This integration gap is a release blocker;
passing private collector tests cannot close it.

The first runtime-clock checkpoint compiles natively but passes only four of
six new fixtures. One fixture passes a pattern to Segment, whose public type
requires a signal. Another incorrectly equates pending collector executions
with published snapshot state. The actual retention function publishes both
occupancy and replay only after its complete batch succeeds. Repair fixtures
through valid public evaluation and actual transaction assertions before
drawing conclusions about clock correctness. Keep the original failed result.

Execution priority is: finish authentic clock and sampled/source relations;
connect immutable source/configuration consumers to the same retained authority;
install production retention before Reserve; enforce the uniform varying-seed
bound; then verify the complete Native/WASM/browser path. Preserve the requested
scope. Acceptance must include a genuine public candidate exercising nested
Part reuse, repeats and instrument-specific edits/effects through actual host
preparation and finite playback/export. Internal phase completion alone is not
the user-visible definition of done.

The immutable bridge review also confirms an identity constraint: frozen route
metadata does not contain the actual execution seed. A consumer must obtain a
complete authenticated execution context from retained authority; it cannot
reconstruct that seed from a node identifier. Root request selection windows and
owner-local canonical cycles are distinct domains. Missing or ambiguous coverage
must fail without VM access. Shared view cloning is shallow; its Debug output
should expose only opaque metadata, not retained callback/evaluator values.

The production acceptance witness must drive actual host preparation. A missing
or foreign retained authority must refuse before upload or Reserve, while a
genuine complete candidate must admit using the same view as later playback.
Varying seeds additionally require the sealed execution guard and aggregate
configuration-group opening bound; a missing cached seed is not proof that
varying repeats are supported.

The bounded runtime-clock phase is now accepted after genuine repairs:355
distinct Rust tests, native/strict lint/WASM/format gates,590 fresh-artifact
editor tests and frontend build pass. Its normal-stack witnesses cover both
legacy reversal depth and an actually evaluated two-hundred-reversal song
pattern at the larger permitted work ceiling. This acceptance supplies affine
and reversal evidence only. Structural sampling and selected-source relations
are the next actual implementation; admission and production handoff remain
open.

Source inspection during sampled-source implementation confirms that raw inner
execution identity excludes its parent sampling relation. Replay observation
copies must therefore bind original inner owner evidence to the current actual
source boundary. Copying the earlier caller relation unchanged is stale timing
authority. Keep original inner affine/reversal frames and issuer START intact;
the new parent link requires its own genuine invocation and returned-source
membership evidence, charged under the same ledger.

Further varying-domain review distinguishes capacity safety from guaranteed
successful evaluation. A work ceiling bounds observations, but subject-use
multiplicity and aggregate connected configuration births remain separate.
Using the default16,384 work ceiling as a physical pool count would reject
ordinary dynamic music and is not useful admission. A sealed aggregate opening
guard can prevent resource excess during first-time seed execution, but does
not prove every permitted seed will succeed. Useful finite-return certificates
must cover all permitted scalar/fixed-pattern alternatives, including returned
Patterns; one observed callback result remains insufficient.

Exact required executions and authenticated symbolic execution domains need
distinct authority. Missing required records still fail before callbacks;
genuine first-time varying-seed execution must not be accidentally classified
as such a missing record. Its issued execution provenance must authenticate
against the immutable cover's domain. An initial immutable ReplayView cannot
silently gain future records, and immutable routing never invokes the VM on a
miss. These are remaining executable integration requirements, not existing
uniform-support claims.

The subsequent sampling review finds a legitimate independently retained
owner-to-sampled-source replay case. Its original q-entry has no parent source
boundary; a later genuine selected-source invocation does. Authenticate the
original owner and the actual current invocation before linking copied owner
observations to that boundary. Preserve original inner frames, issuer START,
raw execution identity and returned membership; never synthesize membership
from notes or change the cache key to evade reuse. Acceptance must prepare
real frozen owner dependencies and deny callback reads during later replay.
The earlier five-of-eight sampling failure included fixture layout/construction
errors. The repaired relation is now verified by genuine independent-owner
reuse with two actual parent bindings and callback reads denied on replay.

The sampling checkpoint passes364 distinct Rust tests, native/strict lint/WASM/
format gates,590 fresh-artifact editor tests and frontend build. All928 source
inputs remain exact after those checks. Local evaluation of Segment before
Slice's lazy index argument preserves its real structured timing; callable
Sound is independently tested. The bounded capture/binding phase is accepted,
while immutable production integration remains unfinished.

The next source-grounded prerequisite is actual invocation retention. Current
occupancy publication discards nested-owner rebound rows, and raw cached inner
executions retain the original entry relation. Immutable geometry cannot treat
those as the later caller's invocation. Retain the full actual key/seed/entry,
original raw execution, actual invocation entry clock and actual bound
observation rows at the genuine query boundary, including sealed empty
successes. An empty observation vector cannot retain its caller relation by
itself. Cached outer executions must preserve and rebind genuinely issued child
invocation templates because they skip nested q entry. Ownership may point only
to already-sealed descendant templates, never its own or ancestor invocation.
Publish transactionally without
callback replay or ownership cycles. Later lookup issues opaque addresses only
from these records. Route events must receive provenance minted while the query
has its actual execution context; static IDs and user event fields cannot
reconstruct that authority. Thread an owned opaque view into route planning and
resolution, then connect density/admission and production pre-Reserve retention.

### Latest review checkpoint

The repaired owner-invocation run passes370 distinct tests, native compilation,
strict Clippy, WASM and formatting gates. The fresh artifact also passes590
editor tests and frontend build, with all930 input hashes unchanged. Historical
test extraction byte equivalence is not independently reproducible from retained
content; the original19 tests pass, which is separate behavioral evidence.
The function/Part architecture remains appropriate. The release blockers remain
actual immutable geometry consumers, query-issued routing provenance, retained
authority established before host Reserve, and useful varying-seed admission.
Earlier fixture failures and their repairs are preserved in the reconciliation
log. Passing collector tests does not establish those production contracts.

The next bounded lookup and geometry companions issue opaque addresses from
published invocations and consume actual source-parent projections. Preserve
original issuer START independently of structural sample-time resets. Nested
owners require the sealed dependency/source chain rather than matching only
the outer request owner. The first geometry consumer requires genuine owning
tests; later route provenance and production handoff remain executable requirements.

The first lookup run passes29 of35 focused tests, including all original25,
and fails six new fixtures. Read-only review identifies incorrect immediate
fixture assumptions about empty q entry, valid ceiling depth, descendant owner
selection and allocation-based frozen Part inventory. Repair through genuine
topology and original authority; timing assertions behind these failures remain
unproven. Geometry also needs an opaque bound-site validation seam and actual
intermediate source-parent START/orientation evidence, rather than relying on
only a final projected footprint and original issuer START. Both are now explicit
geometry deliverables. This is a focused implementation correction, not evidence
for replacing function/Part composition or claiming completed song mode.

The corrected lookup retry executes34 of35 focused tests successfully. The
remaining distinct-use test exhausts cumulative work during redundant probes;
exhaustion is not authority-refusal evidence. Follow-up source review finds a
lookup defect: matching only the first frozen parent allocation can reject a
descriptor owned by a later genuine copy of that parent. Authenticate the exact
owning descriptor copy and its complete use path, preserving foreign-pointer
refusal. This bounded repair and independent retry precede geometry integration.

Measured diagnostic work for the chord/two-use fixture spends15388 of16384
units in retention, leaving996 for request and member lookup. The complete
cumulative witness therefore requires an explicitly configured larger work
allowance; the next test uses32768, already permitted by SongLimits. It keeps
one original counter and all musical/authority assertions. Production defaults,
resource pools and default musical admission remain unchanged and unproven;
this work allowance is not a resource-opening bound.

Production provenance inspection identifies the exact handoff: query.rs
pattern_rows creates the genuine owner invocation around q/replay and finishes
it before event expansion. Snapshot query then copies public FrozenSongEvent
descriptors; Ready.query forwards them and scheduler realize calls resolve_route
with no original execution proof. Retain an opaque query-issued leaf certificate
through that handoff. It must preserve actual seed/entry/source-use context,
cannot be reconstructed from handle or public note fields, and must not point
back to an execution/clock that retains the same returned origins. A copied
descriptor or an unused cloned view does not close this integration requirement.

## Production integration contract: route authority to playback (2026-10-03)

This section is the design baseline for the six-plan batch: immutable route
authority, issued route resolution, shared issued query work, issued playback,
structural clock hooks and reconciliation. It narrows the earlier review notes
into executable boundaries. It does not replace function/Part composition, and
it does not add a new abstraction layer. Earlier sections stay authoritative
where this section is silent.

### Starting state (checkpoint 37ea3e8)

The Route8 slice compiles. Its author compile log
`tmp/song-mode-riela/vactr-route-authority-author-compile-008.log` has 32
warning lines, all dead code. Two of them are outside Route8. The legacy
`prepare_routes` now goes through `prepare_routes_metered`, which calls
`nested::prepare_nested_covers_metered`. That leaves the held0003 compatibility
wrapper `prepare_nested_covers` (`src/song/routing/nested.rs:99`,
`src/song/routing/nested/preparation.rs:3`) used only by tests. It also changes
the legacy nested ledger: held0003 called the wrapper with
`max_nodes = remaining after walk` and depth 0. Every other warning is a new
Route8 item that no production code calls yet.

### Wave order and path ownership

Waves follow dependencies. Paths inside a wave do not overlap. Every path is a
concrete file that the owning plan's manifest declares.

| Wave | Plans | Declared paths |
|---|---|---|
| 1 | Immutable route authority | the exact eight Route8 paths |
| 2 | Issued route resolution; shared issued query work; structural clock hooks (run serially as 2a, 2b, 2c from session 251) | resolver nine (the eight plus `src/song/routing/density/index.rs`, session 251), shared-work four (including `src/pattern/eval/song_replay.rs`), structural six |
| 3 | Issued playback | five paths, including `src/sched/song/pools.rs` |
| 4 | Reconciliation | its manifest (docs, plans, `tests/song_end_to_end.rs`) |

Structural clock hooks are ready for wave 2 because their prerequisites (the
canonical clock, sampling and query authority) are already accepted. Running
them in wave 2 keeps wave 1's cohort an exact 947-input hold. `prepared.rs` and
`lookup/authority.rs` belong to more than one plan. Only one wave edits them at a
time.

A file can only grow past 1000 lines if the plan first declares a cohesive
child path. Today's near-limit files are `song_clock.rs` 994,
`routing/source.rs` 983, `tests/song_end_to_end.rs` 916, `snapshot.rs` 905,
`host/caps/song/preparation.rs` 903, `lookup.rs` 841 and `song_replay.rs` 828.
If wave 3 would push `preparation.rs` past 1000 lines, the playback plan
declares exactly one child (`src/host/caps/song/preparation/issued.rs`) before
release. Nothing else is added implicitly.

### Wave 1: completing immutable route authority

1. **Restore the legacy nested ledger.** The private preparation core takes a
   nested-ledger selection that only `prepare.rs` can see. Legacy
   `prepare_routes` calls `nested::prepare_nested_covers` with held0003
   semantics. `prepare_routes_issued` calls the metered variant with inherited
   limits and depth. This fixes both out-of-manifest warnings without editing
   nested paths. Legacy plans for every existing fixture must stay
   byte-identical.
2. **Input-size precharges.** Before each operation, charge its checked
   upper-bound size against the caller's `remaining`. Failure keeps the debit and
   publishes nothing. The operations are:
   - track/template setup (root tracks times the bus scan);
   - branch-demand aggregation;
   - sidechain scans;
   - `source::validate_detectors` and graph-compilation inputs (nodes, edges,
     traversal and diagnostics);
   - the final topology copy;
   - the publication Capture track search;
   - copy-site and copy-policy binding scans;
   - the transcript invocation find (the measured authentication delta, which
     is transcript length plus 1).

   Document each bound at its call site. The legacy wrapper keeps its fixed
   1,000,000/256 policy. Existing tests are the compatibility witness, and none
   is edited or weakened.
3. **Genuine owning fixtures.** The `#[cfg(test)]`
   `capture_test_route_authority` lives in `route_view.rs`. Bulk assertions live
   in `prepared.rs`. They must cover:
   - real candidate evaluation, retained Index invocations and issued events;
   - dropping PreparedSong and the evaluator, then resolving copied
     site/policy/member geometry with callbacks disabled;
   - refusal of foreign, cloned-and-mutated and swapped site/policy
     references, and of mismatched settings (tempo, tail and seed);
   - exact and one-less work/depth at view issuance, trusted copy and preparation;
   - success and failure through the `with_work` bridge, keeping prior debits.
4. **Dead-code disposition.** No `allow` or `expect` suppression may be added.
   Every Route8 item either has a named consumer below or is deleted in wave 1:
   - `LookupAuthority::Issued`, `with_work` and `bind_issued_owner`: consumed by
     wave-2 resolution through the issued-index operand.
   - `RouteAuthorityView`, the issuers, `TrustedRouteCopy` and the
     site/policy copies: consumed by `prepare_routes_issued`.
   - `PreparedRoutes` and its references: consumed by the wave-2 resolver and
     wave-3 Ready/preparation.
   - `IssuedOwnerSelector` (session 250): consumed by the wave-2 resolver.
   - Private helpers and fields that are reachable only through the items
     above: for example `prepared::invalid` and `PublishedRetainedIndex.site`.
     They share their caller's consumer.

   The Route8 plan's disposition table (rows D01-D32) is the authoritative
   per-diagnostic list. It is checked against
   `tmp/song-mode-riela/session249-route8-clippy.log`.

   Waves 1 and 2 record the strict Clippy run with its exact remaining warning
   list. Only `dead_code` and `unused_imports` on the items listed above may
   remain; any other lint, including `clippy::too_many_arguments`, fails the
   wave. Strict all-target Clippy with `-D warnings` must exit 0 from wave 3
   onward. The Route8 plan becomes Completed only after wave 3 consumes it,
   which matches its own TASK-003 consumer criterion.

   Every new Route8 function has at most seven parameters. `bind_issued_owner`
   meets this by taking its caller selection inputs (prepared site, issuer,
   use-trace prefix and owner window) as one borrowed selector struct. The
   selector only selects a retained request. It grants no authority. This
   grouping does not change authentication semantics:
   - the private retained-request selection;
   - exact retained-execution membership;
   - the owner, placement, seed, entry, depth, recipe and coverage checks;
   - the fresh issued invocation clock;
   - the serialized `with_work` bridge, including the order, amount and
     failure retention of every charge.

   The exact signature is pinned in the Route8 plan (session 250 amendment).
5. **Exact 947 input cohort.** The held0003 cohort has 944 inputs (a path to
   sha256 map in `route-preparation-meter-full-cohort-held-0003.json`).
   `immutable-route-authority-source-intent-0001.json` records
   `expected_full_cohort_after_three_new_children = 947`. The three added inputs
   are `src/song/snapshot/occupancy/route_view.rs`,
   `src/song/snapshot/occupancy/lookup/authority.rs` and
   `src/song/routing/prepared.rs`. The wave-1 receipt lists exactly these three
   additions. It lists changed hashes only for the five existing Route8 paths.
   The other 939 hashes are unchanged.

### Wave 2a: issued route resolution

- **Entry point.** `PreparedRoutes::resolve_issued_event(batch, event_index,
  work, depth)`. It first authenticates that the batch's original Song is the
  same allocation as the view's original Song.
- **Authenticate every contributor before reconciliation.** This covers every
  invocation seal (through `transcript.invocation` plus `bind_issued_owner`) and
  every source contribution. Each contribution's own augmented origin and each
  of its member slots must authenticate against genuine transcript members and
  the attested copied-policy binding. Discarded augmented origins are included.
  Resolving only the surviving descriptor or the first branch is not allowed.
- **Geometry.** Use the fresh rebound issued invocation clock, never the
  retained one. Use the original source START for eligibility, full
  configuration-group identity, and the connected union of uncut projected
  wholes. Clip to the owner only after the union.
- **Conflicts.** Contributors that resolve to conflicting routes fail. The
  resolver returns no route and no partial result.
- **Barriers removed only on the issued path.** The issued children route
  `NeedsJointGeometry` (`nested.rs:293`) and `index_stage_configuration`'s
  static-clock reconstruction into the canonical geometry consumer instead of
  refusing. Public `resolve_route` keeps both barriers and its current outputs.
- **No VM access.** A missing retained record is refused without VM or callback
  access.
- **Songs without Index use.** These resolve through the attested site/policy
  bindings and the existing structural checks. The issued path must not refuse
  a song just because it has no retained Index records.

### Wave 2b: shared issued query work

- `SongSnapshot` and `PreparedSong` gain `query_issued_with_work(span, work,
  depth)`, as declared in the plan. The legacy remaining-taking API wraps them.
- `ReplayView::seed_collection` becomes additive.
  - It first checks that the ledger is attached to the same original Song.
  - It precharges a checked `view.len * (prior.len + 1)`.
  - It appends each view execution that is not already present by `Rc::ptr_eq`.
  - A prior record that is not authentic fails without changing the ledger.
- For a fresh, empty collector, the result and the charge (`view.len`) equal the
  current behavior. The call sites in `occupancy.rs` and `query/issued.rs` keep
  their counts.
- Scalar equality never grants membership. A fresh-only collector is not an
  acceptable substitute for additive retention.

### Wave 2c: structural clock hooks

Follow the plan's six-path manifest:

- Extract dispatch first.
- Euclid passes its actual step timing event to `with_structural_sample`.
- Ply, Arp, Chop, Striate, LoopAt and Fit keep their child clocks unchanged.
  Each of the seven removed barriers needs an evaluated or frozen Index fixture.
  The fixtures must show callback-denied replay with no independent re-reads.
- Chunk keeps its Unknown barrier. Issued resolution then refuses truthfully.

### Wave 3: issued playback

- **Preparation.** In `SongHostPreparation::prepare`, before `resources::build`
  and any Reserve or upload, the steps are: retain the Index occupancy, then
  `issue_route_authority`, then `prepare_routes_issued`. These replace today's
  fixed 1,000,000 charge plus `prepare_routes`. The route stage uses a local
  `u32` counter set to the smaller of the owner's remaining work and 1,000,000.
  The actual debit is written back to `cleanup.remaining` on both success and
  failure. A missing or foreign authority refuses before Reserve.
- **Ready.** Ready owns `PreparedRoutes`. Public `routes()` returns `plan()`,
  and public descriptor `query()` is kept.
- **Scheduler `realize`.** It creates one `SharedIndexWork` per realization
  window from the scheduler limits. It calls `query_issued_with_work` and sorts
  envelopes by onset, then handle. It resolves every envelope and every
  contributor before coalescing. Equal-handle events are coalesced only when
  their descriptors are consistent and their authenticated routes are equal.
  This replaces the handle-only `dedup_by`, the per-event `max_nodes/count`
  allowance and the scalar `resolve_route` in production. No scalar fallback
  remains.
- **`pools.rs`.**
  - A charged staging copy of the projection state is taken before allocation.
  - Assignments and encoded commands go only into the stage.
  - Commit swaps the stage in and pushes the commands only after the whole batch
    succeeds.
  - On any route, encoding, work or capacity failure, the stage is dropped and
    the old pools, queue, cursor and pending receipts stay unchanged.
  - Acknowledgement and generation semantics are unchanged.
- **Varying seeds in static songs.** A static Song has a finite set of
  placements. Pre-Reserve retention executes every `:vary` and `:same`
  placement's first execution under the original ledger. Additive seeding
  (wave 2b) keeps those executions across later queries. Runtime replay
  therefore never needs an unseen seed, and missing records refuse without the
  VM. Evidence needs two `:vary` repeats of a random-dependent Part that differ
  musically while authenticating distinct executions. `:same` must reproduce
  identical events. Runtime first-time execution of seeds unknown at
  preparation is out of scope (see the user-QA file).

### Requirement-level end-to-end evidence

`tests/song_issued_transport.rs` (playback manifest) drives public preparation,
Ready, the production scheduler and native export with one static program. The
program contains:

- reusable Part generator functions, including nested function reuse;
- `sequence`, plus `part-repeat` with both `:same` and `:vary`;
- `part-events` with `delete-event`, and `overwrite-region`;
- `transform-instrument` with `lpf` on one instrument, and `instrument-fx` on
  one instrument branch;
- at least one Index/Slice-sourced track, so retained issued routing is
  exercised.

The test asserts:

- the transport ends on its own with no cycle argument;
- the frame count equals arrangement frames plus tail frames exactly;
- the deleted event is absent and the region content is overwritten;
- only the selected branch has the effect;
- `:vary` repeats differ and `:same` repeats are equal;
- route resolution performs no callback reads;
- export produces a finalized WAV whose metadata matches.

The existing `tests/song_end_to_end.rs` cases must pass unchanged through the
issued scheduler. Helper-only tests do not count as this evidence. Wave 4
re-runs the evidence, records it and updates all six plans.

### Verification gates and evidence

Run each command in the foreground. Write its full log to
`tmp/song-mode-riela/session249-<wave>-<gate>.log` and record the exit status.

- `CARGO_TERM_QUIET=true cargo build`
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` (must pass
  from wave 3 onward; see the warning-list rule for waves 1 and 2)
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
  NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`, the full suite plus a focused
  run with `--test song_route_preparation --test song_source_routes
  --test song_end_to_end --test song_checker`, and `--test
  song_issued_transport` from wave 3
- `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown
  --no-default-features --features host-wasm` (the WASM gate; see below for how
  it was confirmed)
- `rustfmt --edition 2021 --check <touched files>`

The WASM command is confirmed by the held artifact
`target/wasm32-unknown-unknown/debug/vactr.wasm` in
`ROOT-editor-route-meter-20261003.json`, by the `cdylib` crate type, and by the
build command documented in `editor/vite.config.ts`. That command needs
`--no-default-features --features host-wasm`, because the default feature
`host-native` enables native-only dependencies. rustfmt follows `mod`
declarations into child files. The format gate therefore passes when rustfmt
reports no `Diff in` hunk for any touched file. Hunks it reports for untouched
child files are recorded, not fixed. Do not format the whole crate.

Regression baseline: the 425 distinct Rust tests at held0003 must not regress.
The cohort count rises only by declared new files. The projection is 947 after
wave 1, 952 after wave 2, 953 after wave 3 and 953 after wave 4. Each wave
receipt confirms the actual count.

### Unowned working-tree paths (session 250 correction)

Session 249 called the edits to `src/song/snapshot/resources.rs`,
`src/song/snapshot/reservations_tests.rs` and
`src/sched/runtime/song/clock_tests.rs` "pre-existing drift". That was wrong.
The run itself rewrote them, most likely by running rustfmt in write mode on a
parent module outside its writePaths. There is no accepted pre-existing drift, and no cohort
audit may excuse a changed hash with that label.

- No plan owns these three paths. No wave edits, formats, stages or commits
  them, and every wave commit stages its declared paths explicitly.
- rustfmt and cargo fmt run in write mode only on a plan's own Rust writePaths.
  They never run on a parent whose child modules are outside those writePaths.
  Gates use `--check`.
- Every cohort audit, from wave 1 to wave 4, allows changed hashes only on the
  declared writePaths of the plans joined so far. A changed hash on any other
  path fails the audit. The receipt names the path under `unownedChanges`.
  It is not relabelled.
- A `--check` hunk that rustfmt reports for an untouched child module is still
  recorded and not fixed (see the format-gate rule above). That rule covers
  formatting output only. It does not excuse cohort hash changes.
- Before the Route8 gates are rerun, all three paths must equal `HEAD`, which
  is the state the operator described. Under the
  [SM3](../user-qa/pending-song-mode-questions.md#sm3-unowned-rustfmt-rewrites-in-the-working-tree)
  default, the orchestrator does this, never a worker or reviewer. It first
  saves `git diff -- <the three paths>` to
  `tmp/song-mode-riela/session250-unowned-rewrite.patch`, then restores exactly
  those three paths to `HEAD`. It confirms with `git diff --quiet -- <the three
  paths>` (exit 0) and records the result in the Route8 receipt. Workers and
  reviewers never run `git checkout`, `git restore` or `git stash`. If the
  paths still differ, the Route8 cohort gate fails.

### Session 251 resume amendments (2026-10-04)

SONG-ROUTE8 is accepted at `1ac457f` and is not redispatched. Commit
`1ac457f` also holds unreviewed partial wave-2 code. The design above stays
the baseline. These amendments apply only to the remaining waves.

- **Serial wave 2.** Session 250 blocked because three wave-2 workers verified
  one shared, changing tree. Wave 2 now runs as 2a (SONG-ISSUED-RESOLUTION),
  2b (SONG-SHARED-WORK) and 2c (SONG-STRUCTURAL-CLOCK), with concurrency 1.
  Wave 3 and wave 4 follow. Each sub-wave runs all of its gates on a tree that
  no other worker is editing. The orchestrator commits that sub-wave's declared
  paths before the next sub-wave starts. The cohort is already 952 inputs,
  because every new wave-2 file exists at `1ac457f`. It must stay 952 through
  2a-2c.
- **Clippy disposition across sub-waves.** A sub-wave's strict Clippy log can
  show diagnostics from committed partial code that a later sub-wave owns. The
  receipt maps each one to its owner: `SW-<item>` for 2b paths, `ST-<item>` for
  2c paths, `RES-<item>` for 2a paths, or a Route8 D-row. These diagnostics do
  not fail the earlier sub-wave. They are blockers for their owner. For
  example, `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` is
  `SW-let_and_return` in 2a and must be gone after 2b. Any diagnostic in the
  sub-wave's own paths that is not `dead_code`/`unused_imports` on a listed
  item fails that sub-wave. Strict Clippy must still exit 0 from wave 3.
- **Density preflight seam (2a).** The operator authorized
  `src/song/routing/density/index.rs` as a ninth resolver writePath. The
  density walk is shared by the legacy and issued ledgers, and it cannot see
  retained authority. A change there is therefore allowed only if a resolver
  fixture with a statically admissible Index operand is refused by the density
  preflight. The change must keep legacy `prepare_routes` plans byte-identical
  for every existing fixture. It must never turn
  `IndexSupportAdmission::RequiresRealization` into a bound. Dynamic Index
  operands, such as a late-bound function variable in the Index list, still
  refuse at preparation on both ledgers (user-QA SM4 default (a)). If no
  change is needed, the file stays unedited and the receipt says so.
- **Resolver fixtures (2a).** The seven resolver tests use statically
  admissible Index lists. Their callback-denied evidence comes from the Slice
  subject lambda (`{beat -> p}`) and the reusable Part functions. Every Slice
  operand follows the first-structure rule: an unstructured subject and a
  structured Index. The address check in `src/song/routing/index.rs`
  (`slice_index_root`) is a shared legacy validator. It is not a writePath and
  is not relaxed. The distinct-equal-handle fixture is repaired, not the
  contract. The NeedsJointGeometry test must prove that legacy refuses at the
  barrier itself, with the message `sampled context requires joint mapping
  geometry`, and not at preparation. The issued path must resolve the same
  event.
- **Structural clock (2c).** Add one Euclid-over-Slice depth boundary test to
  `structural_tests.rs`. With inherited depth `max_depth - 1`, the query
  succeeds. With inherited depth `max_depth`, it fails with `DepthExceeded`,
  publishes no events and keeps the collector debit.
- **Playback pools path (3).** The fifth playback path is
  `src/sched/song/pools.rs` (`PoolBook`/`Slot`), as declared in the manifest and
  in the wave table above. `src/host/caps/song/preparation/pools.rs` is
  host preparation code. It is not a playback writePath.
- **Evidence.** Before a session-251 sub-wave reruns its gates, the
  orchestrator copies that plan's existing `tmp/song-mode-riela/session249-*`
  logs and receipts to `tmp/song-s249/<planId>/attempt-session250/` and records
  their sha256 values. The plan-declared log paths are then rewritten by the
  new gate runs. Every gate runs in the foreground with its exit status and
  full log path recorded. The no-progress fingerprints `4945fec7...`,
  `15c388bd...` and `0493996e...` must change.
