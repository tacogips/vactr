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
  publishes no events and keeps the collector debit. (Superseded by the
  session 255 searched-boundary contract: `max_depth - 1` is unreachable for
  this fixture.)
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

### Session 252 resume amendments (2026-10-04)

Session 251 blocked in 2a on a retention defect in an accepted Route8 path. The
design above and the session 251 amendments stay the baseline. These amendments
add only the repair contract and the ownership rules the operator authorized.

- **Defect.** `CanonicalIndexRequest::same_execution`
  (`src/song/snapshot/occupancy.rs:143`) compares original, recipe, scope,
  track, revision, root, window and depth. It does not compare issuer or prefix.
  `retain_index_occupancy` uses it to coalesce requests, so a second Slice site
  under the same root and window gets no retained record. The issued selector
  in `bind_issued_owner` (`lookup/authority.rs`) filters records by exact issuer
  and prefix, so it finds no record for that site.
  `distinct_equal_handle_invocations_are_all_resolved` fails for this reason.
  The fixture is not at fault.
- **Execution key and site identity.** There are two identities, and they are
  kept separate:
  - The *execution key* is the current eight-field comparison. One
    `observe_part` execution serves every request with the same key, because
    that execution depends on none of issuer or prefix: its rows are filtered by
    owner only.
  - The *retained site identity* is the execution key plus `issuer` plus exact
    `prefix` equality. This is `Vec` equality, not `trace_matches`.
    `same_execution` is repaired to compare this full identity. Its other caller,
    `replay_site`, therefore refuses a request from a different site with the
    existing `foreign canonical replay address` error.
- **Retention rule.** For each request in the batch, scan retained and pending
  records in the current order, with the current per-record charge
  (`prefix.len + 1`).
  - A record with the same site identity coalesces the request and stops the
    scan. Behavior and charges are unchanged.
  - If no record has the same site identity but a primary record has the same
    execution key, append one *site alias record* for the new site. It does not
    run a second `observe_part` or any VM work. It holds the primary execution's
    observations, calls and invocations, with the invocations as the same `Rc`
    allocations. It copies the primary's `peak_depth`, `vm_instructions` and
    `admitted_depth`. The cached-depth refusal applies first. The alias is
    charged its copy size plus 1 and counted by `limits.check_events`. It is
    published only with the complete batch, as today.
  - Otherwise, execute as today.

  The plan pins whether the alias shares the payload by strong reference or by
  a charged copy. Either form keeps the exact invocation allocations.
- **Each execution is enumerated once.** A site alias is marked as one, and
  the marker is visible through the records iterator of both lookup
  authorities. Enumerations that are not scoped to a site skip an alias's
  invocations and coverage, charging only the existing per-record charge. These
  enumerations are `owner_addresses` (`lookup.rs:95`) and the configuration-row
  loop (`lookup.rs:408`). Their outputs therefore stay identical to the
  coalesced behavior before the repair. Publication (`route_view.rs:352`)
  publishes aliases with the marker so that the site-scoped issued selector
  finds them. The selector is unchanged: it still requires exact issuer, prefix
  and retained-execution membership. Scalar equality still grants no
  membership.
- **Legacy invariance.** Fixtures with one Slice site per execution key create no
  alias. Their records, charges, exact-work assertions and `occupancy.len()`
  assertions are therefore unchanged. Fixtures with several sites per key
  (for example `actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct`)
  gain alias records, but every non-site enumeration returns the same output
  as before. No existing assertion is edited. If an existing assertion changes,
  the sub-wave stops and reports it. The assertion is not adapted.
- **Regression test.** Append one test to the existing
  `src/song/snapshot/occupancy/tests.rs`, so the cohort stays at 952. It retains
  two distinct Slice sites with the same execution key in one batch, and
  asserts:
  - two records with distinct (issuer, prefix);
  - one execution: the alias shares the primary's invocation allocations, and
    the callback journal count matches the single-site count;
  - `owner_addresses` returns the same count as with one site;
  - re-retaining either site adds no record;
  - `replay_site` with the other site's request refuses.
- **2a ownership additions.**
  - Authorization 1 adds two writePaths: `src/song/snapshot/occupancy.rs` and
    `src/song/snapshot/occupancy/tests.rs`.
  - Authorization 2 adds two sharedPaths: `src/song/snapshot/occupancy/lookup.rs`
    (alias skip) and `src/song/snapshot/occupancy/route_view.rs` (alias marker
    publication). `lookup/authority.rs` and `prepared.rs` are already 2a
    writePaths.
  - `occupancy.rs` has unowned child modules, so no rustfmt write mode is run
    on it. Formatting uses `--check` only, under the existing child-hunk rule.
- **Downstream seams (authorization 2).** 2b, 2c, wave 3 and SONG-16 may declare
  any of the eight Route8 paths as concrete sharedPaths when a named defect
  there blocks their requirement. SONG-ROUTE8 tests must stay green with
  unchanged assertions. Each plan author reads the code paths that the plan's
  tests exercise and declares likely seams before dispatch. An undeclared seam
  still terminates the run.
- **Resume criteria.** After the repair, 2a reruns all of its gates.
  `distinct_equal_handle_invocations_are_all_resolved` must pass.
  `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` must also
  pass: legacy refuses with the exact message
  `sampled context requires joint mapping geometry`, and the issued path
  resolves. The 2a evidence fingerprint must differ from `04db7146...`,
  `4945fec7...`, `15c388bd...` and `0493996e...`. The session-251 partial 2a,
  2b and 2c code at `980083a` is reviewed as part of its owning sub-wave. It is
  not presumed accepted. The 2b `SW-let_and_return` repair and the 2c Euclid
  depth boundary test stay required.

### Session 253 resume amendments (2026-10-04)

Session 252 landed the site-alias repair (TASK-004) at `6543273`, but 2a still
fails five resolver tests. The design above and the session 251 and 252
amendments stay the baseline. These amendments add only three things: the
issued member-binding contract from the operator diagnosis
(`tmp/song-mode-riela/session252-root-cause-diagnosis.md`), the test-integrity
restorations, and the ownership these need. Nothing else changes.

- **Defect.** The issued path calls the legacy `bind_member` (`lookup.rs:765`)
  from `canonical_index_configuration_issued` (`canonical.rs:417-428`).
  `bind_member` walks the source boundaries of the Index address invocation,
  which is the consuming stage owner's own invocation.
  `with_source_boundary` (`song_clock.rs:838`) creates each source boundary
  with the consumer clock as its parent. `insert_source` (`song_clock.rs:426`)
  attaches it only to the clocks of source-side owner invocations. The
  outermost owner's chain is therefore always empty, and the call fails with
  `original source membership missing` (`lookup.rs:799`). Legacy callers pass
  a source-side address (`geometry_tests.rs:313-379`, `domains.rs:597-616`,
  `lookup_tests.rs:205-251`). Legacy is therefore correct and stays unchanged.
- **Two policy roles per stage.** The issued path keeps two policies apart:
  - The *stage consuming policy* is
    `prepared.policy(site, stage.identity.policy)`. It selects the matching
    boundary on the source side.
  - The *address parent-use policy* is the policy under which the Index
    address owner is used by its own parent. It is `None` for the outermost
    stage and the enclosing stage's consuming policy for an inner stage. It is
    the value passed to `bind_issued_index`, and through the operand to
    `validate_issued_binding`, `empty_source` and `row_source_window`. When it
    is `None`, the existing check at `canonical.rs:251` still requires the
    address's own boundary chain to be empty. That check is not relaxed.
- **Issued member binder.** Add a new `pub(crate)` function in
  `src/song/snapshot/occupancy/lookup/authority.rs`, an existing 2a writePath.
  It is a child module of `lookup.rs`, so it can reuse `selected_policy`,
  `same_intrinsic_owner` and the `SourceBoundaryRef` methods without growing
  `lookup.rs`. Its inputs are the stage owner frame, the event's seals, the
  transcript, the stage consuming policy's frozen selected source, the subject
  handle, the shared work and the depth. For every seal of the event, it:
  1. takes the fresh `transcript.invocation(seal)` clock, never the retained
     clock, and charges it through the shared-work bridge as the seal loop
     does today;
  2. walks that clock's `retained_sources`;
  3. keeps a boundary only if all four checks hold: `policy_parent()` is the
     same intrinsic owner as the stage owner frame (defined under "Call site
     and order"); `policy_matches` the stage consuming policy;
     `selected_policy` yields use edges; and `member(subject_handle)` yields
     an origin.

  Result rule:
  - Zero matches fail with `original source membership missing`.
  - Matches from more than one seal are accepted only if they name the same
    origin allocation (`std::ptr::eq`) with equal use edges. This covers one
    retained boundary that appears in several seals' chains. The diagnosis
    shows exactly this: the rev2-to-rev3 boundary appears in both seal 1 and
    seal 2.
  - Any disagreement fails with `ambiguous original member binding`.
  - Scalar equality never stands in for allocation identity.

  The binder returns the boundary's original origin and never rebases or clips
  the source START. Legacy `bind_member`, `canonical_index_configuration` and
  `resolve_route` stay unchanged.
- **Call site and order.** In `issued_index_stage_configuration`, the binder
  runs once per timing. It runs after the timing's prefix match and before the
  seal loop that builds configurations, so it always precedes coalescing. If
  the binder fails, the event fails.
  - The *stage owner frame* is the owner identity the binder compares
    `policy_parent()` against. It is the stage owner's intrinsic frame, taken
    from the `lookup_owner()` of the fresh owner-matching invocation, which is
    the same frame the restored owner-frame filter selects. It is obtained
    before the configuration seal loop and needs no bound Index address.
  - The binder still walks the fresh clocks of every seal of the event,
    including non-owner seals, because the source-side boundaries live there.
  - If no seal matches the stage owner frame, the timing fails with the
    existing `no matching fresh invocation` error.

  The call and its seal walk live in a new declared child module,
  `src/song/routing/nested/issued/members.rs`, declared as `mod members;` in
  `nested/issued.rs`. This is needed because
  `nested/issued.rs` is at 943 lines and must stay below 1000.
- **`canonical.rs`.** `canonical_index_configuration_issued` no longer calls
  `bind_member`. Its other checks stay keyed to the address parent-use policy.
  Member evidence is threaded through `configuration.rs` only if the geometry
  consumer needs it. That threading stays within seven parameters, with no new
  `allow` or `expect`.
- **Hard failure restored.** `bind_issued_index` returns `None` only when no
  retained request at the selector's site matches. That test does not depend on
  the seal, so it cannot tell this stage's owner seals from other owners'
  seals. The per-seal owner-frame filter that `1ac457f` had is therefore
  restored ahead of `bind_issued_index`. Session 251 removed it. The fresh
  `transcript.invocation(seal)` owner frame (`lookup_owner()`) must match the
  stage owner on root (`stage.output_owner.payload().id`), track
  (`stage.handle.track()`), revision (`stage.handle.revision()`) and placement
  (`stage.handle.placement()`). A seal that does not match is skipped. It is
  another owner's invocation, and the member binder reads its source
  boundaries. If `bind_issued_index` returns `None` for an owner-matching seal
  (no retained request at the site), that seal is also skipped. An
  owner-matching seal that `bind_issued_index` binds must yield a canonical
  component. If `issued_index_configuration` returns `None` for it, the event
  fails with `issued Index event has no canonical component`, as it did at
  `1ac457f`. The session-251 `continue` on that branch is removed. The
  existing conflict error and the `no matching fresh invocation` error stay.
  If an owner-matching bound seal legitimately has no component, the sub-wave
  stops and reports that seal's owner frame. It does not restore the
  `continue`.
- **Fixture integrity.** Each change below is verified by diffing the test
  module against `6543273` and against `1ac457f`.
  - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` returns to
    its `1ac457f` shape: one Slice under one sampling combinator, and one
    selected event. All four assertions come back:
    1. legacy `prepare_routes` is `Ok`;
    2. the legacy `resolve_route` error contains
       `sampled context requires joint mapping geometry`;
    3. `resolve_issued_event` is `Ok`;
    4. both paths check the same event handle.

    Only two substitutions are allowed:
    - the sampling combinator, Euclid to `segment` or `grid`, under the
      session 252 conditional rule (Euclid stays deferred to 2c);
    - a static Index list, under SM4 default (a).

    The nested-slice replacement and the "some event hits the barrier" loop are
    removed.
  - `discarded_augmented_source_origins_are_resolved` still uses `[cut]`. That
    is a dynamic `Late` operand, which SM4 default (a) refuses on both ledgers.
    This refusal is the observed `Index requires shared canonical realization`
    failure. The fixture's Index list becomes static, and every assertion
    stays, including `found_discarded_augmented`.
    `src/song/routing/density/index.rs` is edited only if the static fixture is
    still refused by the preflight (the session 251 seam rule). If the static
    fixture no longer retains a discarded augmented origin, the sub-wave stops
    and reports it. The test is not weakened.
  - In `cached_nested_slice_events_resolve_from_issued_transcript`, session 251
    changed `[cut nil]` to `[0 nil]`. That is the authorized SM4 substitution,
    and it stays.
  - In `partitioned_nested_issued_queries_equal_the_full_route_set`, session
    251 changed `[cut nil]` to `[0 nil]` and dropped the `fn cut` line. That is
    the same SM4 default (a) static-list substitution, and it stays.
  - In `distinct_equal_handle_invocations_are_all_resolved`:
    - `[cut nil]` to `[0 nil]` is an authorized SM4 substitution, and it
      stays.
    - The added seal-debit assertion strengthens the test, and it stays.
    - `slice p 2` to `slice {beat -> p} 2` (in `inner` and in both `outer`
      stack arms) changes the fixture shape and is not an authorized
      substitution. 2a restores `slice p 2` and keeps the static list. If the
      restored fixture no longer emits a coalesced event with at least two
      distinct invocations, the sub-wave stops and reports. It does not weaken
      the test.
  - No other existing test line in `nested/issued.rs` changes beyond the
    classified changes above.
- **Ownership and cohort.** 2a adds one writePath:
  `src/song/routing/nested/issued/members.rs` (new).
  `src/song/snapshot/occupancy/lookup.rs` stays a sharedPath, and the binder
  does not go there. Besides the session 252 alias skip, its only other edit
  concerns `selected_policy`. That method needs a `RetainedIndexAddress`, which
  the binder does not have, so its body moves verbatim into
  `lookup/authority.rs` as a free function that takes `LookupAuthority`. The
  method keeps its signature and delegates to that function. Legacy behavior
  is byte-identical, and `lookup.rs` gets shorter. The new file raises the
  cohort from 952 to 953 at the 2a join. It stays 953 through 2c, becomes 954
  after wave 3 (955 only if `src/host/caps/song/preparation/issued.rs` is
  needed), and is unchanged after wave 4. These numbers supersede the earlier
  projections. `rustfmt --check` on `nested/issued.rs` now also covers
  `members.rs`, which is owned.
- **Resume criteria.** 2a reruns all of its gates.
  - The five tests pass, together with the session 252 named tests:
    - `cached_nested_slice_events_resolve_from_issued_transcript`
    - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
    - `distinct_equal_handle_invocations_are_all_resolved`
    - `partitioned_nested_issued_queries_equal_the_full_route_set`
    - `discarded_augmented_source_origins_are_resolved`
  - The three legacy fixture groups above pass with unchanged assertions.
  - `grep -n "issued Index event has no canonical component"
    src/song/routing/nested/issued.rs` matches a hard-error path.
  - `grep -n "lookup_owner" src/song/routing/nested/issued.rs
    src/song/routing/nested/issued/members.rs` shows the owner-frame filter in
    the seal loop.
  - The evidence fingerprint differs from the four prior values and from every
    session 252 receipt.

  After 2a is accepted, the serial order holds:
  1. 2b, including the `SW-let_and_return` repair at
     `src/song/snapshot/issued.rs:231` and additive charged retention in
     `src/pattern/eval/song_replay.rs`;
  2. 2c, including the Euclid inherited-depth boundary test;
  3. wave 3, with the staged atomic commit in `src/sched/song/pools.rs`;
  4. SONG-16 requirement-level evidence.

### Session 254 resume amendments (2026-10-04)

Session 253 left `a8b8ed2` with unreviewed partial code for 2a, 2b and 2c and
six failing tests in full nextest. The design above and the session 251-253
amendments stay the baseline. These amendments apply the operator diagnosis
(`tmp/song-mode-riela/session253-root-cause-diagnosis.md`). Where they conflict
with an earlier amendment, they supersede it. SONG-ROUTE8 stays accepted and is
not redispatched.

- **Serial order.** The order is now SONG-STRUCTURAL-CLOCK (2c), then
  SONG-ISSUED-RESOLUTION (2a), then SONG-SHARED-WORK (2b), then
  SONG-ISSUED-PLAYBACK (wave 3), then SONG-16. 2c has no real dependency on 2a
  or 2b: session 249 ran them in parallel, and its writePaths do not overlap
  theirs. 2c goes first because its committed partial code breaks full nextest,
  and a red full suite blocks every other plan's gate. Each step is reviewed,
  accepted and committed before the next starts.
- **Euclid joint-geometry seam dropped.** 2a now runs after 2c. The 2c
  conditional sharedPath on `src/song/routing/nested/issued.rs` and its
  `issued_euclid_joint_geometry_resolves_after_structural_clock` witness
  (session 252/253 notes) are therefore removed. 2a keeps the Euclid program
  already in `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`.
  The legacy barrier this test asserts is the routing `NeedsJointGeometry`
  refusal (`sampled context requires joint mapping geometry`). It is
  independent of the clock dispatch, so it still holds after Euclid's clock is
  instrumented.

#### Structural clock (2c) repair

- **Real builtin arities.** The fixtures are reconciled to the existing
  builtins. No arity is added. `euclid` takes three positional arguments
  (pattern, pulses, steps) plus the `rotation:` keyword, which defaults to 0
  (`src/types/natives_domain.rs:214`, `src/vm/natives/pattern.rs:271-278`).
  `chunk` takes three: pattern, divisions and a function
  (`natives_domain.rs:198`). Changing these signatures would change the public
  language surface, which the plan's non-goals exclude ("No new public
  operators"), and would touch files outside its writePaths. The fixture
  rewrites keep each program's meaning, so no assertion changes:
  - `euclid X 1 2 0` becomes `euclid X 1 2`, and `euclid X 2 4 0` becomes
    `euclid X 2 4`. Rotation is 0 either way.
  - `euclid X 3 8 1` becomes `euclid X 3 8 rotation: 1`. This also applies to
    the session 251 depth-boundary program.
  - `chunk X 2` becomes `chunk X 2 {q -> fast q 2}`, the form
    `tests/song_source_conditionals.rs:100` already uses.
  - `euclid {chop ...} 3 0` has three positional arguments, but it means
    3 pulses over 0 steps. Session 253 evidence shows it fails with
    `euclid steps must be between 1 and 4096`. It becomes
    `euclid {chop ...} 3 8`, which keeps 3 pulses and rotation 0. Its
    assertions are not tied to the step count. (This bullet was corrected at
    plan creation, citing
    `tmp/song-s249/SONG-ISSUED-RESOLUTION/session253/full-nextest-post-helper.log`
    line 570.)
  - That full run stopped at fail-fast after 1229 of 2791 tests. Every 2c
    full-suite gate therefore uses `--no-fail-fast`, so that all failures are
    listed.

  These edits are confined to `src/pattern/eval/song_clock/structural_tests.rs`.
  `src/pattern/eval/song_clock/tests.rs` stays unedited. Line 64 is only where
  the shared harness reports the type error. (Session 255 adds one authorized
  fixture swap in `tests.rs`; see "Session 255 resume amendments".)
- **Euclid is a declared behavior change.** The Euclid barrier is not
  restored. Euclid stays in the instrumented arm of `with_clock_dispatch`
  (`src/pattern/eval/song_clock/dispatch.rs:13`), as Wave 2c intends. The
  observable change is this: a canonical Index consumer over Euclid-sampled
  Slice music now gets a `Known` clock and consumes successfully. Before, it
  refused with `FailCode::Type` because the clock was `Unknown`. Legacy
  `resolve_route` still refuses sampled contexts at its `NeedsJointGeometry`
  barrier, and Chunk stays `Unknown`.
- **`domains.rs` ownership and replacement evidence.** 2c adds
  `src/song/snapshot/occupancy/geometry_tests/domains.rs` as its seventh Rust
  writePath. It is a test file, and this is the only snapshot-side change the
  plan's "no routing, snapshot or replay files" non-goal permits. In
  `unsupported_and_continuous_index_evidence_refuse_without_callback_fallback`:
  - The continuous `range saw` case is unchanged.
  - The Euclid body leaves the refusing branch. A positive branch replaces it.
    Under `deny(snapshot)`, `consume` must return `Ok` for every owner address
    of `request.revision`, with at least one address. The callback read count
    stays 0, and the prior replay `Rc` is unchanged (`Rc::ptr_eq`).
  - The refusing branch keeps every existing assertion: `FailCode::Type`,
    `refused > 0`, the unchanged replay `Rc` and zero reads. It now runs on a
    still-unsupported operator:
    `chunk {slice {beat -> nil} 1 [cut]} 2 {q -> fast q 2}`. If Chunk yields
    no refused address, the sub-wave stops and reports. It does not drop
    `refused > 0`.
  - The test may be renamed only if its new name still says that unsupported
    evidence refuses. The receipt classifies the diff line by line against
    `a8b8ed2`.

  Further replacement evidence comes from `structural_tests.rs`. It covers the
  Euclid `Known` projection, the empty subject, nested orientation, split
  queries and exact/one-less work, plus the Euclid inherited-depth boundary
  test (session 251 contract; superseded by the session 255 D4 projection
  semantics and searched depth boundary).
- **2c gates.** The focused filter adds `test(/geometry_tests::domains/)`.
  Full nextest must exit 0 except for failures that the receipt names as owned
  by 2a, with exact test names. After this repair, the only failures allowed
  are the five 2a resolver tests listed below. Any other failure fails 2c. The
  cohort stays 953. The unchanged rules are the seven-barrier scope, Chunk
  staying `Unknown`, no edits to `combinators/input.rs` or
  `song_clock/tests.rs` (session 255 permits one fixture swap in `tests.rs`),
  and every file staying below 1000 lines.

#### Issued resolution (2a) decisions

- **Owner-frame predicate (supersedes session 253 "Hard failure restored").**
  `owner_matches` (`src/song/routing/nested/issued/members.rs:5-10`) took
  `root` from the stage output owner but `track`, `revision` and `placement`
  from `stage.handle`. That handle is the stage *input* event
  (`issued.rs:247-262`), so the four fields named two different owners and
  never matched. All four fields now come from the stage *output*:
  - `root` stays `stage.output_owner.payload().id`;
  - `track`, `revision` and `placement` come from the stage output handle. For
    stage 0 that is `event.handle`. For stage `k > 0` it is
    `stages[k-1].handle`.

  `event.handle` is passed into `members::issued_index_stage_configuration`.
  The predicate keeps all four fields and adds no scalar shortcut.
- **Retained execution versus site request (the "not retained at site"
  decision).** In `bind_issued_owner_if_matching`
  (`src/song/snapshot/occupancy/lookup/authority.rs:409-518`), two facts are
  checked separately, and both must hold:
  1. *Site request exists.* The current selector scan is unchanged. It
     requires an authenticated record with this site's scope, track, exact
     issuer and exact prefix, a window covering the owner window, and the same
     original payload allocation. If there is none, the result is `Ok(None)`,
     as today.
  2. *Execution authentically retained.* The fresh seal's execution must be
     retained, with exact `Rc` identity through
     `transcript.authentic_retained_invocation`, by a retained invocation in
     *any* record of the route authority view. Every such record belongs to
     the same original Song, because the view has exactly one. That record
     must pass `authenticate_authority`. Its retained invocation must equal
     the fresh one on owner frame, seed and entry, which are the existing
     comparisons. Identity is never relaxed. Value-equal, handle-equal or
     scalar-equal executions grant nothing, and the depth check stays.

  The bound request is chosen as follows. If a site-matching record itself
  retains the execution, that record's request is bound, exactly as today. So
  every currently passing path keeps its binding. Otherwise the first
  site-matching request in record order is bound, provided fact 2 holds. If
  fact 1 holds and fact 2 fails, the call still fails with
  `required issued execution is not retained at site`. The added scan over
  non-site records is charged at the existing per-record and per-invocation
  rates through the same `ProjectionBudget` and counted by
  `limits.check_events`. The legacy lookup binding in
  `src/song/snapshot/occupancy/lookup.rs` and `resolve_route` do not change.

  Why: issued replay can serve an inner Slice site with the execution retained
  under the enclosing record. The diagnosis observed the stage-1 seal
  `rev2/[5,2]` recognized only by the outer record (scope 4, invocation 5),
  while the inner-site records at scopes 2 and 3 hold separate executions.
  Requiring both facts in one record would refuse a genuine execution.
  Dropping either fact would accept foreign or unretained work.
- **Duplicate frozen `inside` (investigation, not a fix).** 2a records in its
  receipt why the selector used scope 2 while the selected sources came from
  part 3. If parts 2 and 3 are separate placements of the same reusable
  function, each owns its own frozen copy. That is expected, and the binding
  must not merge them by value. If instead the stage output scope is computed
  wrongly, that is a 2a defect in `nested/issued.rs` or `members.rs`, fixed
  there. Either outcome is recorded with its owner frames. No change outside
  2a's paths follows from it.
- **Fixtures (supersedes the session 253 fixture-integrity bullets where they
  differ).**
  - `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`: one
    `PreparedSong` supplies the legacy plan and event, the retained route
    authority (`PreparedSong::issue_retained_route_authority`,
    `route_view.rs:488`) and the issued batch
    (`PreparedSong::query_issued_with_work`, `snapshot.rs:306`). The
    test no longer evaluates the program twice through `capture` and
    `prepared_song`, so revisions match. All four assertions stay. The Euclid
    program stays.
  - `distinct_equal_handle_invocations_are_all_resolved`: use
    `slice {beat -> p} 2 [0 nil]` in `inner` and in both `outer` stack arms.
    This restores the session 251 amendment. `slice p 2` has a structured
    subject, which `slice_index_root` (`src/song/routing/index.rs:172-173`)
    correctly refuses, and that rule is not relaxed. The distinct-seal
    selection, the `len() >= 2` assertion and the seal-debit assertion stay.
  - `discarded_augmented_source_origins_are_resolved`: this is a declared
    assertion change with replacement evidence. The handle-based
    `found_discarded_augmented` predicate cannot be satisfied. Every frozen
    contribution is value-equal to the descriptor origin, and a discarded
    origin is visible only by allocation identity in live rows. The fixture
    (static `[0]`) stays. The test selects an event with at least two source
    contributions that carry slice timing, asserting that one exists, and
    resolves it with `resolve_issued_event`. It then asserts that the work
    debit is at least the contribution count. This mirrors the seal-debit
    check. Two pieces of evidence replace the old assertion:
    1. this stricter per-contribution debit;
    2. the unchanged snapshot test
       `fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority`
       (`src/song/snapshot/issued.rs:551-595`). It proves by `Rc::ptr_eq` that
       the same equal-handle union shape holds a discarded allocation with
       post-seal timing.

    The test may be renamed to describe the union. `density/index.rs` stays
    unedited, and the receipt says so.
  - `cached_nested_slice_events_resolve_from_issued_transcript` and
    `partitioned_nested_issued_queries_equal_the_full_route_set` keep their
    static lists and every assertion.
- **2a acceptance.** The five tests pass: `cached_nested`,
  `partitioned_nested`, `issued_joint_geometry`, `distinct_equal` and
  `discarded_augmented` (or its renamed form). The session 252 named tests and
  the legacy fixture groups pass with unchanged assertions. Full nextest is
  green. The authority change is a 2a edit of `lookup/authority.rs`, which is
  already a 2a writePath. The cohort stays 953.

#### Carried forward unchanged

- 2b: additive charged authentic retention in
  `src/pattern/eval/song_replay.rs`. No fresh-only collector is allowed. 2b
  also fixes `clippy::let_and_return` at `src/song/snapshot/issued.rs:231`
  without `allow`/`expect`.
- Wave 3: the production scheduler consumes issued routes, and
  `src/sched/song/pools.rs` stages projections and commands, then commits
  atomically.
- SONG-16: requirement-level end-to-end evidence and the editor npm test and
  build.
- Gates, the unowned-path rules (`resources.rs`, `reservations_tests.rs` and
  `clock_tests.rs` equal `HEAD`; `.agents/settings.local.json` untouched), and
  `--check`-only formatting are unchanged. Session 254 evidence fingerprints
  must differ from every prior receipt. Gate logs keep the plan-declared
  `tmp/song-mode-riela/session249-*` names. Prior copies go to
  `tmp/song-s249/<planId>/attempt-session253/`.

### Session 255 resume amendments (2026-10-04)

Session 254 stopped in 2c at `c7083fb` with four failing structural tests. The
operator diagnosis (`tmp/song-mode-riela/session254-structural-diagnosis.md`)
found no production defect. All four failures, plus one older test, encode
wrong expectations for a sampling Euclid. The design above and the session
251-254 amendments stay the baseline. Where these amendments conflict with an
earlier one, they win. SONG-ROUTE8 stays accepted and is not redispatched. The
serial order is unchanged: 2c (SONG-STRUCTURAL-CLOCK), 2a
(SONG-ISSUED-RESOLUTION), 2b (SONG-SHARED-WORK), wave 3
(SONG-ISSUED-PLAYBACK), then SONG-16. The dispatch manifest
`impl-plans/active/song-s249-dispatch.json` is updated in place with a
`resumeSession255` entry. It is never duplicated.

#### Implementer authority: fix within writePaths

Six consecutive runs ended when the implementer stopped at the first
behavioral failure. For every remaining plan (2c, 2a, 2b, wave 3, SONG-16),
this rule replaces each earlier "stop and report" clause that does not fall
under the three stop conditions below.

- **Default.** The implementer diagnoses each failing gate and fixes it inside
  the plan's writePaths and declared sharedPaths. This covers production code
  and the plan's own tests. The implementer reruns the focused gate until the
  plan's declared requirements pass, then runs every gate. Each fix is recorded
  in the receipt under `fixes[]`, with:
  - the failing test and its exact message;
  - the root cause;
  - the edited paths;
  - the class: `production-defect` or `own-test-corrected`, and for the
    latter, the design rule it now matches.
- **Stop only for:**
  1. a fix that needs a path that is neither a writePath nor a declared
     sharedPath of the plan;
  2. a change to a *pre-existing baseline assertion* that no accepted design
     decision covers;
  3. a security or identity check that would have to be relaxed. Examples:
     `Rc`/allocation identity, the owner/seed/entry/site authentication, the
     hard `issued Index event has no canonical component` failure, any depth
     check in production code, and the first-structure rule in
     `slice_index_root`.
- **Definitions.**
  - A *pre-existing baseline assertion* is an assertion in a test whose
    function name already exists at `37ea3e8`, the checkpoint before wave 2.
  - The plan's *own tests* are tests that a plan added after `37ea3e8`. They
    include the session 249-254 additions in `structural_tests.rs`,
    `nested/issued.rs`, `occupancy/tests.rs` and `shared_work_tests.rs`.
  - Correcting an own test toward accepted design semantics is not weakening.
    Every proof obligation the test carried must remain, either as the same
    assertion or as a replacement that is equal or stronger and is named in
    the receipt. Deleting an own test, or reducing it to `is_ok()`, is
    weakening.
  - Accepted design decisions that cover pre-existing assertions are these:
    the session 254 `domains.rs` change, the session 254
    `discarded_augmented` replacement evidence, and the session 255
    `tests.rs:478` fixture swap below.
- **Retained stop clauses.** These earlier clauses map to stop conditions 2
  or 3, so they stay:
  - the session 252 legacy-invariance rule: an existing occupancy assertion
    that changes stops the sub-wave;
  - the session 253 rule: an owner-matching bound seal with no canonical
    component stops the sub-wave rather than restoring `continue`;
  - every "never relax" rule.

  Every other "stop and report" or "stop and record a blocker" clause in the
  six plans now means "diagnose and fix within writePaths, recording the
  diagnosis".
- **Seams up front.** Each plan's session 255 amendment lists its concrete
  sharedPaths before dispatch. These are file paths, never directories. The
  plan author reads the code that the plan's tests exercise. The seams known
  now are:
  - 2c adds the writePath `src/pattern/eval/song_clock/tests.rs`, for one
    fixture string only;
  - 2a keeps its writePaths and its sharedPaths `lookup.rs`, `route_view.rs`,
    `occupancy.rs` and `occupancy/tests.rs`. Growth in
    `src/song/routing/source.rs` (991 lines) goes into the existing child
    `src/song/routing/source/issued.rs`, and `source.rs` stays at no more than
    993 lines;
  - 2b keeps its conditional seams `src/song/snapshot/occupancy.rs` and
    `src/song/query/issued.rs`;
  - wave 3 keeps the conditional `src/song/routing.rs` re-export seam.

  A seam found only during implementation is stop condition 1.

#### D4: Euclid sampling projection semantics

D4 (keep Euclid instrumented) stands. This subsection states what an
instrumented sampling Euclid projects. It follows the existing rule in
"Immutable consumers and authentic clock capture": first-structure sampling
reassigns a sampled point to a new structural whole.

- Euclid point-samples its child once per pulse step. It passes the step's
  timing event to `with_structural_sample`, which records one
  `ClockBoundary::Sample` relation per sampled step. That relation holds the
  step whole, the part and the sample point.
- When `project_for` crosses that boundary, it does three things:
  - it replaces the projected whole with the step whole;
  - it sets the projected `sample_start` to the step's begin;
  - it resets orientation to `At`.

  A row is applicable only if its inner projected whole contains the sample
  point. Frames inside the sample (Fast, Rev, weighted steps, Chop) project
  first and can make a row non-applicable. For example, a Rev under a point
  sample reflects the Index whole `0..1/2` to `1/2..1`, which misses point 0.
- The row's own `issuer_sample_start` is unchanged. It is the original issuer
  START, which is kept separately from structural sample-time resets.
- The six time-preserving operators (Ply, Arp, Chop, Striate, LoopAt, Fit) add
  no sample boundary. Their rows project to the unchanged Index whole, issuer
  START, orientation `At`, and every row is applicable.
- An empty-subject Slice still records its Index row. Under Euclid, only the
  pulse steps are sampled. An empty subject therefore yields exactly one row
  per sampled pulse step, each projecting to its step whole. A *false
  observation* means a row beyond those rows. It does not mean any row at all.
- Collector rows are scratch, not publication. "Publishes no events" on a
  refused query is checked through the owning snapshot, as
  `song_clock/tests.rs:500-502` already does. It is never checked through
  collector emptiness.
- No change to `src/pattern/eval/song_clock.rs`,
  `src/pattern/eval/song_clock/dispatch.rs` or
  `src/pattern/combinators/structure.rs` follows from this subsection. The
  code already behaves this way (`structure.rs:403-418`,
  `song_clock_projection.rs:102-109`).

#### Structural clock (2c) test corrections

These five corrections are own-test corrections or authorized fixture swaps.
The diagnosis verified each one in a scratch worktree.

- **Euclid-only projection helper.** Add a private helper to
  `structural_tests.rs`. For every target-owner row it asserts:
  1. the clock is `Known`;
  2. `row.clock.sampling_evidence()` has exactly one relation, and that
     relation's whole is `Some(step)`;
  3. the footprint from `project_for(&row.owner, whole,
     row.issuer_sample_start, work)` is `Some`, with
     `footprint.whole == step` and `footprint.sample_start == step.begin`;
  4. `footprint.orientation == ClockOrientation::At`.

  It also asserts that the row set is non-empty and that at least one row is
  applicable. It replaces `assert_unchanged_projection` for every Euclid
  program:
  - the Euclid entry of `seven_structural_operators_preserve_slice_clocks`;
  - `split_queries_retain_the_single_query_index_rows` (`2 4`, steps 1/4
    wide);
  - `nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation`
    (`3 8`), which keeps its extra check that some row carries sampling
    evidence;
  - the accepted case of the depth test.

  The six time-preserving entries and
  `striate_ranking_reuses_retained_work_without_callback_reads` keep
  `assert_unchanged_projection` unchanged.
- **Empty subject.** `euclid_empty_subject_has_no_false_index_observation`
  asserts:
  - the query succeeds;
  - the rows are exactly the sampled pulse-step rows. For `1 2` over cycle 0
    that is one row, a hand-computed constant with a comment;
  - every row passes the Euclid helper.

  `rows(work).is_empty()` is removed. The exact count is the stronger
  replacement.
- **Searched inherited-depth boundary.** This replaces the session 251
  `max_depth - 1` contract, which this fixture cannot reach: the run peaks at
  depth 8, and the cut VM callback needs one more. Measured: 247 is `Ok`; 248
  fails with `canonical VM inherited depth exhausted`; 255 fails with `pattern
  nesting too deep`; 256 fails with `canonical prerequisite topology depth
  exceeded`. All of them are `FailCode::DepthExceeded`. The program stays
  `euclid {slice {beat -> p} 2 [cut nil]} 3 8 rotation: 1`, and each probe
  uses a fresh `DepthFixture`.
  - **Search.** Probe from `max_depth - 1` downward, at most 32 probes. Every
    refused probe must fail with `FailCode::DepthExceeded`; any other error
    fails the test. B is the first depth that succeeds. The test asserts that
    B was found. B is not hard-coded, and the receipt records it.
  - **At B.** The result is `Ok`, the Euclid helper passes, and the VM
    `song_work()` is cleared.
  - **At B + 1.** The result is `FailCode::DepthExceeded`, `remaining() <
    max_nodes` (the debit is kept), and the VM `song_work()` is cleared. There
    is no collector-emptiness assertion; see "Collector rows" under D4.
  - **At `max_depth`.** An extra refusal: `FailCode::DepthExceeded`, collector
    observations empty (measured 0, because the topology check fails before
    any collection), and the VM `song_work()` is cleared.
  - No production depth check moves or relaxes.
- **Pre-existing `song_clock::tests` fixture swap.** In
  `unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling`
  (`src/pattern/eval/song_clock/tests.rs:478`), replace exactly one string,
  `euclid {slice {beat -> p} 2 [0]} 2 2`, with
  `chunk {slice {beat -> p} 2 [0]} 2 {q -> fast q 2}`. This keeps the
  unsupported-sampling coverage on Chunk, which is still `Unknown`. Every
  assertion, the test name and the line count (896) stay unchanged.
  `git diff c7083fb -- src/pattern/eval/song_clock/tests.rs` shows exactly one
  changed line. `tests.rs` becomes a 2c writePath for this edit only. It is
  hand-edited and never run through rustfmt in write mode.
- **Production unchanged.**
  `git diff c7083fb -- src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/dispatch.rs src/pattern/combinators/structure.rs`
  is empty. If a corrected test still fails, the implementer fixes the test
  toward this contract. If it could pass only by editing one of those three
  files, that is outside this run's accepted scope: stop and report.
- **2c gates.**
  - The focused filter `test(/song_clock|combinators|geometry_tests::domains/)`
    must exit 0. It includes the swapped `song_clock::tests` test.
  - Full nextest with `--no-fail-fast` must complete. Its only allowed
    failures are a subset of the five 2a resolver tests named in session 254.
    Any 2c-path failure is fixed in 2c. Any other failure is stop condition 1
    and is reported with its owner.
  - Strict Clippy shows no diagnostic in a 2c path.
  - WASM must pass.
  - `rustfmt --check` must show no `Diff in` hunk on a changed line of a 2c
    path. A pre-existing hunk on unchanged lines of `tests.rs` is recorded and
    not fixed.
  - Every file is below 1000 lines. The cohort stays 953.

#### Remaining waves under the new authority

- **2a.** The session 254 decisions stand unchanged: the owner-frame
  predicate against the stage output handle, the single-`PreparedSong`
  joint-geometry fixture, the separate site-request and retained-execution
  facts with exact `Rc` identity, `slice {beat -> p} 2 [0 nil]`, the
  equal-handle union with a per-contribution work-debit assertion, and no
  `density/index.rs` edit. The five resolver tests and full nextest
  (`--no-fail-fast`, zero failures) must pass. The implementer fixes failures
  within the 2a paths under the authority rule above. The issued path
  authenticates all contributors, and each source contribution's augmented
  origin and member slots, before coalescing. It uses fresh rebound clocks,
  the original source START, full configuration groups and connected uncut
  wholes before clipping. Legacy `resolve_route` and its
  `NeedsJointGeometry` barrier stay.
- **2b.** Unchanged. Seeding in `src/pattern/eval/song_replay.rs` keeps the
  accumulated executions additively, with charged, authentic retention, and
  no fresh-only collector. `clippy::let_and_return` at
  `src/song/snapshot/issued.rs:231` is fixed without `allow`/`expect`.
- **Wave 3.** Unchanged:
  - the production scheduler `src/sched/song.rs` consumes issued routes
    through `PreparedRoutes::resolve_issued_event`, with no `dedup_by` and no
    scalar `resolve_route`;
  - `src/sched/song/pools.rs` (the playback plan's fifth path) stages
    projections and commands and commits only after the whole batch succeeds;
  - a failure test asserts that the pools, queue, cursor and pending receipts
    are unchanged.

  Strict Clippy exits 0 from this wave on.
- **SONG-16.** Unchanged. `tests/song_issued_transport.rs` and the existing
  `tests/song_end_to_end.rs` are the requirement-level evidence. They cover
  reusable Part functions, `sequence` and `part-repeat` (`:same`/`:vary`),
  `delete-event`, `overwrite-region`, instrument-selective `lpf` and
  `instrument-fx`, automatic termination, and the WAV export.
- **Evidence.** Before rerunning, each plan copies its prior
  `tmp/song-mode-riela/session249-<plan>-*` files to
  `tmp/song-s249/<planId>/attempt-session254/` with sha256 values. Every new
  receipt fingerprint differs from every earlier one. Gates run in the
  foreground, and each records its exit status and full log path. The
  unowned-path rule (`resources.rs`, `reservations_tests.rs` and
  `clock_tests.rs` equal `HEAD`; `.agents/settings.local.json` untouched) and
  `--check`-only formatting are unchanged.

### Session 256 resume amendments (2026-10-04)

Session 255 stopped at `88f5120`. SONG-STRUCTURAL-CLOCK (2c) finished its test
corrections, but full nextest had six failures and the 2c disposition allowed
only five. The design above and the session 251-255 amendments stay the
baseline. Where these amendments conflict with an earlier one, they win.
SONG-ROUTE8 stays accepted and is not redispatched. The serial order is
unchanged: 2c, 2a (SONG-ISSUED-RESOLUTION), 2b (SONG-SHARED-WORK), wave 3
(SONG-ISSUED-PLAYBACK), then SONG-16. The dispatch manifest
`impl-plans/active/song-s249-dispatch.json` gets a `resumeSession256` entry in
place and is never duplicated. The operator diagnosis is
`tmp/song-mode-riela/session255-source-fixture-diagnosis.md`.

#### 2c acceptance: exact allowed failures

- **Six allowed failures.** This replaces the session 255 rule "a subset of the
  five 2a resolver tests". The 2c full-suite gate (`--no-fail-fast`) may fail
  only on these exact test IDs, all owned by 2a:
  1. `song::routing::nested::issued::tests::cached_nested_slice_events_resolve_from_issued_transcript`
  2. `song::routing::nested::issued::tests::partitioned_nested_issued_queries_equal_the_full_route_set`
  3. `song::routing::nested::issued::tests::issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
  4. `song::routing::nested::issued::tests::distinct_equal_handle_invocations_are_all_resolved`
  5. `song::routing::nested::issued::tests::discarded_augmented_source_origins_are_resolved`
  6. `song::routing::source::issued::tests::genuine_source_contributions_reject_a_foreign_transcript`

  The sixth test fails identically at `1ac457f` and `c7083fb`, before any 2c
  change. Its fixture has no Euclid or child-sampling operator. Any failure
  outside this list fails 2c. A missing listed failure is not an error, but the
  receipt records it. The session 255 run
  (`tmp/song-mode-riela/session249-structural-nextest-full.log`) shows exactly
  these six with 2787 passed and 3 skipped.
- **No new implementation.** 2c reruns its session 255 gates once on the final
  tree. The gates are build, strict Clippy with disposition rows, the focused
  filter, full nextest, WASM, `rustfmt --check` on 2c paths, line counts and
  the cohort. 2c then goes to test-integrity, adversarial and integration
  review. A gate failure in a 2c path falls under the session 255 implementer
  authority rule. Clippy disposition rows owned by later plans stay as
  declared; `let_and_return` at `src/song/snapshot/issued.rs:231` stays owned
  by 2b.

#### 2a: source fixture correction

- **Own-test classification.** `src/song/routing/source/issued.rs` first
  appears in `1ac457f`, after `37ea3e8`. Its tests are 2a's own tests, and
  the file is a 2a writePath. Correcting the fixture is an `own-test-corrected`
  fix, not a baseline-assertion change.
- **Root cause.** The `SLICE` subject never uses the transform's selected
  source `p`, so `base` is never queried. Only events that pass through the
  selected source carry source contributions
  (`src/song/snapshot/issued.rs:28`, `:175-191`; `:481-484` asserts that
  other events have none). Every event therefore has empty
  `source_contributions()`. The guard in the test (`source/issued.rs:312-313`)
  is correct and stays.
- **Fix.** Replace only the `SLICE` constant in the test module:
  - the `indexed` body becomes `slice {beat -> p} 2 [cut nil]`;
  - the chord moves into the source part:
    `let base {part [drums: {s :analog > chord [:c :five]}] duration: 2}`.

  `fn cut`, `selected` and the `song` line stay. This is the nested resolver
  fixture shape and keeps the function-subject rule in `slice_index_root`.
  `[cut nil]` is the chosen list. `[0 nil]` is not used, so the fixture keeps
  its callable list element.
- **Unchanged assertions.** The non-empty contribution guard stays. The genuine
  transcript must authenticate. `authenticate_contributions` with the foreign
  batch's transcript must still fail. `PLAIN` and the other two
  `source::issued` tests do not change.
- **2a acceptance.** The six tests above pass, and the session 254 and 255 2a
  decisions stand. All three `source::issued` tests pass. Full nextest
  (`--no-fail-fast`) has zero failures. `source.rs` stays at no more than 993
  lines. Issued-path authentication of all contributors and of each source
  contribution's augmented origin and member slots before coalescing is not
  relaxed.

#### Allowed failures for later plans

Each plan enumerates its allowed failures from a full `--no-fail-fast` run at
plan time. After 2a ends green, no known failure remains. 2b, wave 3 and
SONG-16 therefore allow zero full-suite failures. Strict Clippy exits 0 from
wave 3 on, as already declared.

#### Out of scope

`impl-plans/active/song-mode-issued-query-authority.md` is not in this run's
plan set or dispatch manifest. It is neither dispatched nor edited.

#### Evidence

Before rerunning, each plan copies its prior
`tmp/song-mode-riela/session249-<plan>-*` files to
`tmp/song-s249/<planId>/attempt-session255/` with sha256 values. Scratch logs
go to `tmp/song-s249/<planId>/session256/`. Every new receipt fingerprint
differs from all earlier ones. Gates run in the foreground and record exit
status and full log path. These rules are unchanged:

- `resources.rs`, `reservations_tests.rs` and `clock_tests.rs` equal `HEAD`;
- `.agents/settings.local.json` stays untouched;
- formatting runs with `--check` only.

### Session 257 resume amendments (2026-10-04)

Session 256 stopped at `259f0b9`. The 2c acceptance rerun made no Rust edits.
Its focused gate, native and WASM builds and route regressions passed. Full
nextest (`tmp/song-mode-riela/session249-structural-nextest-full.log`) ran 2793
tests: 2786 passed, 3 skipped and 7 failed. Six failures are the allowed 2a
tests listed in session 256. The seventh is
`song_export::fractional_song_and_tail_write_exact_partial_block`, which
panicked at `tests/song_export.rs:20` with `AlreadyExists`. That failure is a
pre-existing test-harness flake, not a regression. The design above and the
session 251-256 amendments stay the baseline. Where these amendments conflict
with an earlier one, they win. SONG-ROUTE8 stays accepted and is not
redispatched. The serial order is unchanged: 2c, 2a, 2b, wave 3, then SONG-16.
The dispatch manifest `impl-plans/active/song-s249-dispatch.json` gets a
`resumeSession257` entry in place and is never duplicated.

#### 2c: temp-directory collision fix (operator authorization)

- **Root cause.** `Directory::new` in `tests/song_export.rs` (lines 10-22)
  names its directory `vactr-song-export-{SystemTime nanos}-{NEXT}` and creates
  it with `std::fs::create_dir`. nextest runs each test in its own process, so
  the static `NEXT` counter restarts at 0 in every process. macOS wall-clock
  time has microsecond granularity. Two tests that start in the same
  microsecond therefore build the same name, and the second `create_dir` fails.
- **Scope of the scan.** Five test files call `SystemTime::now`:
  - `tests/song_cli.rs` (lines 8-21) has the same pattern
    (`vactr-song-cli-{stamp}-{NEXT}`, no process id, `create_dir`). It is a
    latent instance of the same flake;
  - `tests/cli.rs`, `tests/song_assets.rs` (lines 433-442) and
    `tests/song_candidate.rs` (lines 370-378) already include
    `std::process::id()`. They are not changed.
- **Decision.** 2c adds two writePaths, `tests/song_export.rs` and
  `tests/song_cli.rs`, for this fix only. Both are declared so that a known
  flake cannot block a later full-suite gate.
- **Fix.** In each file, only the directory name in `Directory::new` changes.
  It gains `std::process::id()`, for example
  `vactr-song-export-{pid}-{stamp}-{n}` and `vactr-song-cli-{pid}-{stamp}-{n}`.
  Concurrent processes have distinct ids, and the counter still separates
  directories within one process.
  - `create_dir` stays; it is not replaced with `create_dir_all`. A real
    collision therefore still fails loudly instead of sharing a directory.
  - The `Drop` cleanup, every test body and every assertion stay unchanged.
  - `git diff 259f0b9 -- tests/song_export.rs tests/song_cli.rs` touches only
    the `format!` call inside `Directory::new`.
  - Both files are hand-edited and checked with `rustfmt --check`. They are
    never formatted in write mode.
- **Classification.** The receipt records this under `fixes[]` as class
  `harness-collision`. The fix changes no assertion, so it is not a
  baseline-assertion change. These two files carry no song-mode production
  behavior. They are not 2c's own tests for the purpose of the session 255
  authority rule, and any further edit to them is stop condition 1.

#### 2c acceptance rerun

- 2c reruns its session 255 gates once on the final tree. The gates are build,
  strict Clippy with the declared disposition rows, the focused filter, full
  nextest, WASM, `rustfmt --check` on 2c paths (including the two test files),
  line counts and the cohort.
- The full-suite gate (`--no-fail-fast`) may fail only on the six exact
  2a-owned test IDs listed in session 256. A missing listed failure is
  recorded, not an error. Any other failure fails 2c. This includes any
  `song_export` or `song_cli` failure, because the flake is now fixed.
- There is no further 2c production or own-test implementation. After the
  rerun, 2c goes to test-integrity, adversarial and integration review.

#### Later waves

- **2a.** The session 256 source fixture correction stands unchanged: the
  `SLICE` subject becomes `slice {beat -> p} 2 [cut nil]`, the chord moves into
  `let base {part [drums: {s :analog > chord [:c :five]}] duration: 2}`, and
  both authentication assertions stay. The session 253 diagnosis and the
  session 254 and 255 2a decisions stand. 2a ends with all three
  `source::issued` tests and the five nested resolver tests passing, and with
  full nextest at zero failures.
- **2b, wave 3 and SONG-16.** Unchanged from session 256. They allow zero
  full-suite failures. 2b fixes `clippy::let_and_return` at
  `src/song/snapshot/issued.rs:231`. Strict Clippy exits 0 from wave 3 on.

#### Evidence

Before rerunning, 2c copies its prior
`tmp/song-mode-riela/session249-structural-*` files to
`tmp/song-s249/<planId>/attempt-session256/` with sha256 values. Later plans do
the same when they rerun. Scratch logs go to
`tmp/song-s249/<planId>/session257/`. Every new receipt fingerprint differs
from all earlier ones. Gates run in the foreground and record exit status and
full log path. These rules are unchanged:

- `resources.rs`, `reservations_tests.rs` and `clock_tests.rs` equal `HEAD`;
- `.agents/settings.local.json` stays untouched;
- formatting runs with `--check` only.

### Session 258 resume amendments (2026-10-04)

Session 257 stopped at `e71d726`. 2c finished its implementation, including the
harness fix. Full nextest
(`tmp/song-mode-riela/session249-structural-nextest-full.log`) ran 2793 tests:
2787 passed, 3 skipped and 6 failed. The six failures are exactly the 2a IDs
listed in session 256, and there is no `song_export` or `song_cli` failure. 2c
was not accepted, because the workflow progress gate rejects every non-zero
behavioral test exit, whatever allowed-failure list a plan declares. The design
above and the session 251-257 amendments stay the baseline. Where these
amendments conflict with an earlier one, they win. SONG-ROUTE8 stays accepted
and is not redispatched. The dispatch manifest
`impl-plans/active/song-s249-dispatch.json` gets a `resumeSession258` entry in
place and is never duplicated.

#### Serial order and dependency edge

- **Order.** SONG-ISSUED-RESOLUTION (2a), then SONG-STRUCTURAL-CLOCK (2c)
  formatting and review, then SONG-SHARED-WORK (2b), then SONG-ISSUED-PLAYBACK
  (wave 3), then SONG-16. This supersedes the session 254 order. Each plan is
  reviewed, accepted and committed before the next starts. `maxConcurrency`
  stays 1.
- **Edge removed.** The `SONG-ISSUED-RESOLUTION` `dependsOn` list becomes
  `["SONG-ROUTE8"]`. This is safe for three reasons:
  - The 2c code is already committed at `e71d726`, so 2a builds on it. The
    Euclid program in `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier`
    uses the Euclid instrumentation that is already in the tree.
  - The 2a and 2c writePaths do not overlap.
  - All six remaining failures are in 2a writePaths.

  2b, wave 3 and SONG-16 keep their `dependsOn` lists. 2c still precedes them.
  The `SONG-STRUCTURAL-CLOCK` `dependsOn` list becomes
  `["SONG-ROUTE8", "SONG-ISSUED-RESOLUTION"]`. That edge encodes the 2a -> 2c
  order, so 2c's zero-failure gate never runs on a tree where 2a is still
  red.
- **Base commits.** 2a starts from `e71d726`. Each later plan starts from the
  accepted commit of the plan before it. These commits replace the
  `<2c-join>` placeholder. The unowned-path check compares against the plan's
  own base commit. The base commit only sets the starting tree and the
  unowned-path check. It does not narrow what is reviewed. Each plan's review
  (test-integrity, adversarial and integration) and its receipt cover the
  plan's full implementation diff, `git diff 1ac457f <plan-accepted> --
  <plan writePaths and edited sharedPaths>`, where `1ac457f` is the
  `SONG-ROUTE8` accepted commit. For 2c this includes all of its committed
  work since `1ac457f`, not only the format-only hunks. For 2a it includes
  its session 251-253 work that is already in `e71d726`.
- **No allowed failures anywhere.** Every plan's full-suite gate
  (`--no-fail-fast`) must exit 0 with zero failures. This replaces every
  allowed-failure list, including the session 256 and 257 six-ID list for 2c.
  The suite runs without `--retries` and is not re-run until it happens to
  pass. A failure outside the running plan's writePaths and sharedPaths is stop
  condition 1, reported with its owner.

#### 2a: verified current state and fix map

No new 2a design decision is needed. At `e71d726` the session 253 code
(`members.rs`, `mod members;`, the hard `issued Index event has no canonical
component` error) is present. The session 254 decisions (TASK-008 to TASK-011)
and the session 256 source fixture (TASK-012) are not yet applied:

- `owner_matches` (`src/song/routing/nested/issued/members.rs:5-10`) still
  reads `track`, `revision` and `placement` from `stage.handle`.
- `distinct_equal_handle_invocations_are_all_resolved`
  (`src/song/routing/nested/issued.rs:699`) still uses `slice p 2 [0 nil]`.
- `discarded_augmented_source_origins_are_resolved` (`:630-655`) still uses the
  handle-based `found_discarded_augmented` predicate.
- The `SLICE` constant (`src/song/routing/source/issued.rs:241`) is still the
  old subject.

Each failure maps to an accepted decision:

| Failure (exact message) | Tests | Accepted decision |
| --- | --- | --- |
| `issued Index timing has no matching fresh invocation` | `cached_nested_slice_events_resolve_from_issued_transcript`, `partitioned_nested_issued_queries_equal_the_full_route_set`, `issued_joint_geometry_resolves_where_legacy_keeps_its_barrier` | Session 254: the owner-frame predicate uses the stage output handle; the site-request and retained-execution facts are separate, with exact `Rc` identity; joint geometry uses a single `PreparedSong` |
| `Index address contradicts first-structure rule` | `distinct_equal_handle_invocations_are_all_resolved` | Session 254 fixture `slice {beat -> p} 2 [0 nil]`; `slice_index_root` is not relaxed |
| panic at `nested/issued.rs:654` | `discarded_augmented_source_origins_are_resolved` | Session 254 declared replacement: per-contribution debit plus the unchanged `fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority` |
| `issued route source: Slice fixture produced no source contribution` | `genuine_source_contributions_reject_a_foreign_transcript` | Session 256 `SLICE` fixture correction; both authentication assertions stay |

If a test still fails after these decisions are applied, the session 255
implementer authority applies inside the 2a writePaths and sharedPaths. The
session 255 seams stand: `lookup.rs`, `route_view.rs`, `occupancy.rs`,
`occupancy/tests.rs`, and `source/issued.rs` as the only outlet for
`source.rs` growth. A fix that needs a 2c path is stop condition 1.

**2a acceptance:**

- the six tests and the session 252 and 254 named tests pass;
- the focused binaries (`song_route_preparation`, `song_source_routes`,
  `song_end_to_end`, `song_checker`) pass unchanged;
- full nextest exits 0 with zero failures;
- `source.rs` is at most 993 lines, `nested/issued.rs` at most 990, and every
  file is below 1000;
- the cohort is 953;
- strict Clippy diagnostics are limited to the declared later-plan disposition
  rows, with `let_and_return` at `src/song/snapshot/issued.rs:231` still owned
  by 2b;
- issued-path authentication of all contributors, and of each source
  contribution's augmented origin and member slots before coalescing, is not
  relaxed. The issued path uses fresh rebound clocks, the original source
  START, full configuration groups and connected uncut wholes before clipping.
  Legacy `resolve_route` keeps its `NeedsJointGeometry` barrier.

#### 2c: formatting fix and review on the green tree

- **Authorized edit.** 2c may change only the two `rustfmt --check` hunks in
  its own writePath `src/pattern/eval/song_clock/structural_tests.rs`. Both are
  recorded in `tmp/song-mode-riela/session249-structural-fmt.log`:
  - near line 30, the `DecodedSongAssetFactory::new(...)` call is joined into
    rustfmt's two-line form;
  - near line 95, the `song_observation` import is wrapped, and the
    `crate::vm::query_vm::MeteredSongQuery` import moves after
    `crate::value::intern::intern_kw`.

  The file is hand-edited to rustfmt's output for those hunks. No other token,
  assertion or test changes. `git diff <2a-accepted> --
  src/pattern/eval/song_clock/structural_tests.rs` touches only those lines.
  The receipt records the edit in `fixes[]` with class `format-only`.
- **Gates.** `rustfmt --edition 2021 --check
  src/pattern/eval/song_clock/structural_tests.rs` prints no `Diff in`. The
  other 2c paths keep the session 255 rule: no hunk on a changed line, and a
  pre-existing hunk on unchanged lines of `tests.rs` is recorded but not fixed.
  The other session 255 to 257 gates rerun once on the tree after 2a. Full
  nextest exits 0 with zero failures. Strict Clippy shows no diagnostic in a 2c
  path. The other disposition rows stay as declared until wave 3.
- **Review.** 2c then goes to test-integrity, adversarial and integration
  review. There is no other 2c implementation.

#### Later waves

- **2b.** Unchanged: charged, authentic, additive retention in
  `src/pattern/eval/song_replay.rs`, with no fresh-only collector, and
  `clippy::let_and_return` at `src/song/snapshot/issued.rs:231` fixed without
  `allow` or `expect`.
- **Wave 3.** Unchanged:
  - `src/sched/song.rs` consumes issued routes;
  - `src/sched/song/pools.rs` stages projections and commands and commits only
    after the whole batch succeeds, with the old state preserved on failure.

  Strict Clippy exits 0 from wave 3 on. That removes the remaining Route8
  dead-code rows (`LookupAuthority::Issued`, `with_work`, `bind_issued_owner`,
  `PreparedRoutes`) through production use, not through `allow`.
- **SONG-16.** Unchanged requirement-level end-to-end evidence:
  `tests/song_issued_transport.rs` and `tests/song_end_to_end.rs`, plus the WAV
  export. The final rustfmt `--check` list and the cohort allowance keep
  `tests/song_export.rs` and `tests/song_cli.rs`.
- **Route8 cohort.** The held exact-947 Route8 cohort stays documented in
  `impl-plans/active/song-mode-immutable-route-authority.md`. SONG-ROUTE8 is not
  reopened.

#### Evidence

Before rerunning, each plan copies its prior
`tmp/song-mode-riela/session249-<plan>-*` files to
`tmp/song-s249/<planId>/attempt-session257/` with a `sha256.txt`. Scratch logs
go to `tmp/song-s249/<planId>/session258/`. Earlier attempt and session
directories are never deleted or overwritten. Every new receipt fingerprint
differs from all earlier ones. Gates run in the foreground and record exit
status and full log path. These rules are unchanged:

- `resources.rs`, `reservations_tests.rs` and `clock_tests.rs` equal `HEAD`;
- `.agents/settings.local.json` stays untouched;
- formatting runs with `--check` only.

### Session 259 resume amendments (2026-10-04)

Session 258 stopped at `81c68e7`. 2a fixed five of its six failures. Only
`song::routing::nested::issued::tests::discarded_augmented_source_origins_are_resolved`
still fails. It reports `event does not match admitted route topology`
(`src/song/routing/source/issued.rs:160`), because the nested resolver under it
fails with `issued Index timing has no matching fresh invocation`. The operator
diagnosis found that the fixture is correct and that three stacked code defects
cause the failure. Each defect masked the next one. The reference delta is
`tmp/song-mode-riela/session258-operator-fix-delta.diff`. It touches only
`src/song/routing/nested/issued/members.rs` and
`src/song/snapshot/occupancy/lookup/authority.rs`, and both are 2a writePaths.
The delta is a guide. The implementer owns the final code and its evidence.

The design above and the session 251-258 amendments stay the baseline. Where
these amendments conflict with an earlier one, they win. SONG-ROUTE8 stays
accepted and is not redispatched. The serial order from session 258 is
unchanged: 2a, then 2c formatting and review, then 2b, then wave 3, then
SONG-16. The dispatch manifest `impl-plans/active/song-s249-dispatch.json` gets
a `resumeSession259` entry in place and is never duplicated.

#### 2a decision 1: owner revision from the output scope part

This supersedes the `revision` field of the session 254 "Owner-frame
predicate", the session 253 "Hard failure restored" revision source, and the
dispatch manifest TASK-008 wording "compares ... revision ... with the stage
OUTPUT handle".

- **Defect.** `owner_matches` (`members.rs:5-14`) compares `frame.revision`
  with `output_handle.revision()`. For stage 0 the output handle is
  `event.handle`, and its revision belongs to the song root. In the failing
  fixture that root is the sequence (rev 4), while the transform owner is rev 2,
  and its placement `[1,1,5,2]` does match. Earlier nested fixtures passed only
  because their transform was the song root.
- **Rule.** The revision is the revision of the stage's output scope part:
  `prepared.plan().topology.parts[output_scope].revision`. It is read once, next
  to where `output_scope` is computed, with a missing part failing as an
  invalid issued output scope. It is passed to both `stage_owner_frame` and
  `owner_matches`. The two must always use the same value.
- **Unchanged fields.** `root` stays `stage.output_owner.payload().id`.
  `track` and `placement` still come from the stage output handle
  (`event.handle` for stage 0, `stages[k-1].handle` for stage k). The predicate
  keeps all four fields and adds no scalar shortcut.

#### 2a decision 2: cross-seal member agreement (operator-authorized)

This supersedes the session 253 "Issued member binder" result rule for matches
from *different* seals, and the manifest TASK-005 wording "duplicates must have
the same origin (std::ptr::eq)". It is a stop-condition-3 class change, and the
operator authorized it explicitly. No other identity check is relaxed.

- **Defect.** The equal-handle union in the fixture has two base invocations.
  Each one seals its own member allocation for the same handle, with identical
  policy edges `[0,0,0]`. The cross-seal `std::ptr::eq` test rejected them as
  ambiguous. They are exactly the discarded and the surviving contributors that
  this design requires to *both* authenticate.
- **Rule in `bind_issued_member`.**
  - *Within one seal.* Unchanged. All matches must name the same origin
    allocation (`std::ptr::eq`) with equal edges. Otherwise the call fails with
    `ambiguous original member binding`.
  - *Across seals.* Each seal's own binding must have edges equal to those of
    the first bound seal. Origin allocations may differ. Disagreement fails with
    `conflicting original member bindings`.
  - *Zero matches.* Unchanged: `original source membership missing`.
- **Retained authentication (must not change).** For every seal:
  1. The clock is the fresh `transcript.invocation(seal)` of the authenticated
     issued transcript, charged through the shared-work bridge. It is never the
     retained clock.
  2. The four boundary filters stay: `same_intrinsic_owner`, `policy_matches`,
     `selected_policy_in` and `member(handle)`.
  3. `SourceBoundaryRef::member`
     (`src/pattern/eval/song_clock_projection.rs:227`) looks the handle up only
     in that boundary's sealed `returned` set. It validates the genuine origin
     with `copy_origin` and keeps its `ambiguous sealed source member` check.

  A copied or value-equal DTO member therefore still cannot bind, because it is
  not in any sealed `returned` set. The relaxation only lets two genuine sealed
  allocations of one union both authenticate.
- **Use of the returned origin.** `bind_timing_member` uses the binder only as
  an authentication gate and drops the result (`members.rs:141`). The returned
  origin is the first bound seal's allocation. It must not stand in for any
  other contribution's origin. Per-contribution origin identity stays with
  `authenticate_contributions` (`src/song/routing/source/issued.rs:166`), which
  this decision does not change.
- **Review.** The 2a adversarial review covers this rule explicitly. It tries a
  foreign transcript, a copied member DTO, edge disagreement across seals, and a
  within-seal duplicate allocation.

#### 2a decision 3: owner-local Index windows

- **Defect.** `issued_index_stage_configuration` passes song-time windows to
  `issued_index_configuration` (`members.rs:317-318`). In the failing fixture
  those are `[1/2..2]` and `[1/2..5/2]`. Retained rows are owner-local: issuer
  START 0, footprint `[0..2]`. So `row_source_window`
  (`src/song/snapshot/occupancy/lookup.rs:278`) rejects the row, and the event
  fails with `issued Index event has no canonical component`. Earlier fixtures
  hid this because their offset was 0.
- **Rule.** This mirrors legacy `issuer_clock`
  (`src/song/routing/nested/clock.rs:17`) and the legacy add-back
  (`src/song/routing/nested.rs:557-567`).
  1. Compute `local_owner = owner - context.offset` once per call, using checked
     arithmetic.
  2. Pass `intersect(stage.scope.interval, local_owner)` and `local_owner`.
  3. Add `context.offset` back to each returned component before the
     conflicting-routes comparison and the cross-timing `intersect`.

  `context.offset` is the branch offset for every stage, as in legacy.
- **Coverage gap.** The fixture covers a nonzero offset only at stage 0. No
  fixture has an inner source at a nonzero scope offset at a stage with index
  greater than 0. Adding such a 2a own test in `nested/issued.rs` is optional.
  If added, it must keep `nested/issued.rs` at 990 lines or fewer. Otherwise the
  receipt records the gap as residual risk. The gap does not gate acceptance.

#### Plan and manifest text

The plan step amends this text in step with the code:

- `impl-plans/active/song-mode-issued-route-resolution.md`: TASK-008 title and
  body (revision source), the TASK-005 result rule near lines 1020-1025, and
  the identity bullets near lines 1181 and 1199. Each bullet is narrowed to
  "within one seal".
- `impl-plans/active/song-s249-dispatch.json`: a new `resumeSession259` entry
  that restates TASK-005 and TASK-008. Earlier entries stay as history and are
  not rewritten.

#### Base and review range

- 2a starts from `81c68e7`. Its unowned-path check compares against
  `81c68e7`.
- The review and receipt range stays
  `git diff 1ac457f <plan-accepted> -- <plan writePaths and edited sharedPaths>`.
- The receipt records sessions 258 and 259, `baseCommit` `81c68e7`, `fixes[]`
  for decisions 1-3 (class `production-defect`), and a fingerprint that differs
  from every earlier receipt.
- Each later plan's base is the accepted commit of the plan before it, as in
  session 258.

#### 2a acceptance (adds to session 258)

- `discarded_augmented_source_origins_are_resolved` passes with its session 254
  assertions, together with the other focused issued tests (the operator
  scratch run showed 10/10, including two occupancy guards).
- Full nextest (`--no-fail-fast`, no `--retries`) exits 0 with zero failures.
- `nested/issued.rs` is at most 990 lines, `source.rs` at most 993, and every
  touched file is below 1000. The cohort is 953.
- `rustfmt --check` passes on the touched 2a files. The long
  `stage_owner_frame(...)` call in the reference delta is hand-wrapped to
  rustfmt's form.
- There are no new `allow` or `expect` attributes.
- `git diff 81c68e7 -- src/pattern/eval/song_clock.rs src/pattern/eval/song_clock/dispatch.rs src/pattern/combinators/structure.rs`
  is empty. This keeps the 2c frozen-path check valid.

#### Later waves

The later waves are unchanged from session 258:

- **2c.** Fixes the two `rustfmt` hunks in `structural_tests.rs`, reruns its
  gates on the green tree, then goes to review.
- **2b.** Additive charged authentic retention in `song_replay.rs`, plus the
  `let_and_return` fix at `src/song/snapshot/issued.rs:231`.
- **Wave 3.** `src/sched/song.rs` consumes issued routes, and `pools.rs` uses a
  staged atomic commit.
- **SONG-16.** Requirement-level end-to-end evidence.

Strict Clippy exits 0 from wave 3 on. Evidence follows the session 258 rules,
with prior files copied to `tmp/song-s249/<planId>/attempt-session258/` and
scratch logs in `tmp/song-s249/<planId>/session259/`.

2a has one exception. The session 258 implementer already wrote its logs to
`tmp/song-s249/SONG-ISSUED-RESOLUTION/session259/`. That directory is session
258 evidence and stays as it is. 2a writes its session 259 scratch logs to
`tmp/song-s249/SONG-ISSUED-RESOLUTION/session259-resume/` instead.

### Session 260 resume amendments (2026-10-04)

Session 259 stopped at `10c3eab`. Integration review accepted SONG-ROUTE8,
SONG-ISSUED-RESOLUTION, SONG-STRUCTURAL-CLOCK and SONG-SHARED-WORK, and their
code is committed in `10c3eab`. They are not redispatched. Two plans remain:
SONG-ISSUED-PLAYBACK (wave 3, partial code in `10c3eab`), then SONG-16. The
run failed at redispatch because the integration findings needed paths outside
the playback writePaths. This amendment closes that gap by declaring every
likely seam up front.

The design above and the session 251-259 amendments stay the baseline. Where
this amendment conflicts with an earlier one, it wins. The dispatch manifest
`impl-plans/active/song-s249-dispatch.json` gets a `resumeSession260` entry in
place and is never duplicated. Earlier entries stay as history.

#### Playback fix 1: overwrite-region owner duration

- **Defect.** Issued export fails with `foreign source child scope` at beat 4
  (`src/pattern/eval/song_provenance.rs:608`). The predicate
  `source_owns_owner` (`src/pattern/eval/song_clock.rs:192-257`) finds the
  `overwrite-region` replacement payload, then requires the edited Part's full
  duration (4) to equal the owner frame's duration (1). A region payload's
  owner frame is built by `pattern_rows` (`src/song/query.rs:604-627`) with
  duration = region length and offset = region begin, as the "Editing
  contract" requires (the replacement is queried over `[0, end-begin)` and
  shifted by `begin`). So no region payload can ever match. The issued inputs
  (rebound clock, original source START, entry trace, selector) are correct.
  Diagnosis: `tmp/song-mode-riela/session259-foreign-child-scope-diagnosis.md`.
- **Rule.** Inside `source_owns_owner`, in the `PartNode::Edit` arm:
  - `OverwriteRegion` is split out of the shared match. When its track is the
    selected track, it sets the payload and an expected payload duration of
    `region.duration()?` (the `TimeSpan::duration` failure propagates).
  - `ReplaceTrack` and `TransformInstrument` keep the payload with no
    payload duration.
  - The final check becomes
    `payload_duration.unwrap_or(part.duration()) == owner.duration`.
  - The revision, root, track, entry and selector checks, the recursion, the
    work charge and the depth check do not change.
- **Scope.** This is the only authorized change to `song_clock.rs`.
  `git diff 10c3eab -- src/pattern/eval/song_clock.rs` touches only the body of
  `source_owns_owner`. The session 259 frozen-path check for 2c
  (`git diff 81c68e7 -- src/pattern/eval/song_clock.rs` is empty) applied to
  the 2a diff only, and this rule replaces it for the playback diff.
  `src/pattern/eval/song_clock/dispatch.rs`,
  `src/pattern/eval/song_clock/structural_tests.rs` and
  `src/pattern/combinators/structure.rs` stay unchanged.

#### Playback fix 2: export engine profile

- `src/song/export.rs` `render` (around line 188) builds its engine with
  `crate::host::song_profile::song_engine_config(options.sample_rate as f32,
  MAX_BLOCK, caps, StoreKind::NativeArc, 2)`. The `ConfigError` is mapped to a
  `Failure`. The `config.bus_slots = 32` override and the unused `EngineConfig`
  import are removed.
- Export then uses the same engine profile as native song playback: 50 free bus
  slots after the master and 128 template slots
  (`src/host/song_profile.rs`). Diagnosis:
  `tmp/song-mode-riela/session259-bus-slots-diagnosis.md`.
- Not changed: the generic `EngineConfig` bus-slot default, the two-generation
  reservation for nonzero tails (`src/song/routing/prepare.rs:189-211`), the
  `SongPreparationLimits` that export passes, and the export frame
  arithmetic.

#### Playback checks to close (session 259 integration findings 2, 4, 5 and 6)

Each check is a test that runs in the full suite. None of them uses `allow` or
`expect`.

| Check | Where | Input -> expected outcome |
| --- | --- | --- |
| Atomicity | `src/sched/song/realize_tests.rs` (new; declared from `src/sched/song.rs` under `#[cfg(test)]`) | A `realize` batch fails after at least one assignment or command has entered the stage -> pools, pending queue, cursor and pending receipts equal their pre-`realize` values, and no command was pushed. The preferred trigger is real pool-capacity exhaustion on a later event in the same batch. If no fixture reaches it, a `#[cfg(test)]`-only fault point in `pools.rs` staging may be used instead. A second `realize` without the fault produces the same output as a transport that never failed. |
| Authority | `src/sched/song/realize_tests.rs` and `src/host/caps/song/preparation/issued.rs` tests | A `FrozenIssuedBatch` from another `PreparedSong` given to Ready `resolve_issued` -> refusal, and the scheduler state is unchanged. Preparation with a missing or foreign route authority -> refusal before Reserve, and the actual work debit is written back. |
| Ended | `tests/song_issued_transport.rs` | The requirement program -> the transport reaches `Ended` with no cycle argument, and total frames = arrangement frames + tail frames exactly. |
| Bit-exact | `tests/song_issued_transport.rs` | The requirement program exported twice to two files -> the two WAV files are byte-identical, and the header frame count equals the report's `total_frames`. |

- **TASK-003 consumption (finding 4).** `src/song/snapshot.rs:191`
  (`SongSnapshot::query_issued_with_work`) and `:306`
  (`PreparedSong::query_issued_with_work`) are already reached from production:
  `src/sched/song.rs:443-458` creates one `CanonicalIndexCollector`
  (`SharedIndexWork`), passes it to Ready `query_issued_with_work`
  (`src/host/caps/song/preparation.rs:923`), and passes the same collector to
  `resolve_issued` for every event. So `snapshot.rs` needs no edit. The
  evidence is a `realize_tests.rs` test that asserts the debit is cumulative:
  one realize window charges query and every resolution to that collector, and
  the exact budget passes while one less refuses with the old state preserved.
  The playback receipt cites these lines. The worker then ticks TASK-003 in
  `impl-plans/active/song-mode-shared-issued-query-work.md`. The existing
  `cfg_attr(not(test), allow(dead_code))` attributes on the non-work
  `query_issued` (`snapshot.rs:181`, `:296`) predate this plan and stay.
- **Evidence copies (finding 5).** Both `tmp/song-mode-riela/session249-playback-*`
  files (`build.log` at 11:06, `receipt.json` at 11:18) were written by session
  259, after its baseline (`session259/baseline-full.log`, 11:03). Before
  session 259, no playback file existed, so `attempt-session258/` had nothing
  to copy. Session 260 does not create it retroactively. The receipt records
  `attemptSession258: {"files": [], "reason": "no session249-playback-* files
  existed before session 259"}`. Before it reruns, session 260 copies the
  current `session249-playback-*` files to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session259/` with `sha256.txt`.
- **Scratch directory.** `tmp/song-s249/SONG-ISSUED-PLAYBACK/session260/` and
  `tmp/song-s249/SONG-SHARED-WORK/session260/` already hold session 259
  scratch logs. They stay as they are. This run writes its scratch logs to
  `tmp/song-s249/<planId>/session260-resume/`.
- **Measured capacity (finding 6).** The playback receipt records
  `capacity.busSlots`: `required` (read from `SongRoutePlan.required.bus_slots`
  and checked against `tracks + 1 + sum(reserved_generations)`), `available`
  (the engine's free bus slots under `song_engine_config`, measured, expected
  50), the generic-profile value it replaces (31), and `engineProfile:
  "song_engine_config"`. It also records the measured preparation work against
  the 1,000,000 route-stage allowance (the default for
  [SM1](../user-qa/pending-song-mode-questions.md#sm1-index-retention-work-under-production-default-limits)).
  The session 259 fixture needs 31 bus slots. It must not be shrunk further,
  and every requirement bullet stays.

#### Playback seams (bounded checkpoint amendment)

`writePaths` gains exactly:

- `src/pattern/eval/song_clock.rs` (fix 1 only);
- `src/song/export.rs` (fix 2 only).

`src/sched/song/realize_tests.rs` is already in the manifest writePaths. It is
added to the plan's own JSON so the two agree.

`sharedPaths` gains the following. Each is edited only for a concrete defect
that a required playback test reproduces. Each edit is recorded in the
receipt's `fixes[]` with the failing test and message, and stays inside the
named purpose:

| Path | Owner | Purpose if needed |
| --- | --- | --- |
| `src/pattern/eval/song_provenance.rs` | SONG-ISSUED-RESOLUTION | The child-scope check that consumes `source_owns_owner`. 978 lines, so growth must stay at or below 999. |
| `src/song/snapshot.rs` | SONG-SHARED-WORK | A TASK-003 consumer seam. No edit is expected. |
| `src/song/snapshot/issued.rs` | SONG-SHARED-WORK | A strict-Clippy diagnosis in issued batch code. |
| `src/host/caps/song/preparation/pools.rs` | playback-adjacent (unowned) | The preparation-side pool view, if staged commit needs it. |
| `src/host/song_profile.rs` | unowned | Only if `song_engine_config` is not reachable from `export.rs` as is. |
| `src/song/routing/nested/issued/members.rs`, `src/song/snapshot/occupancy/lookup/authority.rs` | SONG-ISSUED-RESOLUTION | Only if the required fixture reaches a resolution defect (see "Known resolution defect" below). |

`src/song/routing.rs` and `impl-plans/active/song-mode-shared-issued-query-work.md`
stay sharedPaths. `src/host/caps/song/preparation.rs` (941 lines) does not grow
past 999. New Ready or preparation code goes into the declared child
`src/host/caps/song/preparation/issued.rs`. A defect in any other path is stop
condition 1, reported with its owner and the exact failing test.

These three paths stay unowned and equal to `HEAD`:

- `src/song/snapshot/resources.rs`;
- `src/song/snapshot/reservations_tests.rs`;
- `src/sched/runtime/song/clock_tests.rs`.

`.agents/settings.local.json` stays untouched.

#### Known resolution defect (not in scope)

The bus-slot diagnosis records that a larger fixture variant (`duration: 2`
with `part-events p :drums 0 2`) fails resolution with `required issued
execution is not retained at site`
(`src/song/snapshot/occupancy/lookup/authority.rs`,
`bind_issued_owner_if_matching`). The session 259 required fixture does not
reach it, and no accepted requirement names that variant. This batch does not
fix it unless a required test reaches it, and then only under the sharedPaths
rule above. The final receipt lists it as residual risk.

#### SONG-16: requirement song through the production CLI

- **Single program source.** SONG-16 adds
  `examples/song-mode/requirement-song.vact`. Its text is exactly the
  `PROGRAM` constant of `tests/song_issued_transport.rs` as accepted for wave 3,
  written with real newlines and tabs. In `tests/song_issued_transport.rs`,
  SONG-16 replaces only that constant with
  `include_str!("../examples/song-mode/requirement-song.vact")`. No assertion
  changes. The library-level checks (deleted event absent, region overwritten,
  only the selected branch filtered or effected, `:vary` repeats differ, `:same`
  repeats equal, no callback reads during resolution, `Ended`, bit-exact
  export) and the CLI check therefore run on one program.
- **CLI test.** The new file `tests/song_requirement_cli.rs` runs the built
  binary (`CARGO_BIN_EXE_vactr`) as
  `vactr render examples/song-mode/requirement-song.vact <tmp>/song.wav
  --sample-rate 8000`, from the crate root and with no cycle argument. Its
  temp-directory helper follows `tests/song_cli.rs`, with the
  process-id + time + counter name from session 257. It asserts:
  - exit status 0 and an empty stderr apart from `vactr: warning:` lines;
  - stdout `rendered N frames at 8000 Hz` with `state Ended`, where the test
    computes `N` from the public `PreparedSong` of the same file:
    `frames_at(duration * seconds_per_cycle + tail_seconds)`. `N` is never a
    copied literal;
  - the WAV length is `44 + N * 4`, and the body has nonzero samples;
  - a second render to another path is byte-identical.
- **Capacity.** The program fits the song profile (measured need 31 of 50 free
  bus slots). The final receipt records the measured need and capacity. Songs
  over capacity are still refused truthfully
  ([SM5](../user-qa/pending-song-mode-questions.md#sm5-song-bus-capacity-for-finite-export)).
- **SONG-16 seams.** `writePaths` gains `tests/song_requirement_cli.rs` and
  `examples/song-mode/requirement-song.vact`. `sharedPaths` gains
  `tests/song_issued_transport.rs` (the `PROGRAM` replacement only),
  `src/song/export.rs` and `src/cli/render.rs`. The last two are edited only
  for a concrete defect that the CLI test reproduces. A limit change there
  follows SM1 option (a) and is recorded with the measured work.
- **Final gates.** These are unchanged from the reconciliation plan, with
  `tests/song_requirement_cli.rs` added to the focused nextest list and the
  rustfmt `--check` list.

#### Gates (each in the foreground, with exit status and full log path)

Playback logs go to `tmp/song-mode-riela/session249-playback-<gate>.log`.
SONG-16 logs go to `tmp/song-mode-riela/session249-final-<gate>.log`. Each must
exit 0:

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
   NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast`, the full
   suite with no `--retries`
4. Focused: the same command with `--test song_route_preparation --test
   song_source_routes --test song_end_to_end --test song_checker --test
   song_export --test song_issued_transport --test song_cli`, plus
   `--test song_requirement_cli` for SONG-16. A `song_clock` lib filter run
   also confirms the structural clock tests.
5. `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown
   --no-default-features --features host-wasm`
6. `rustfmt --edition 2021 --check <touched Rust files>`, under the format-gate
   rule in "Verification gates and evidence"

Strict Clippy exits 0 from playback on. It does so through production use,
with no new `allow` or `expect`
(`git diff 10c3eab | grep -E '^\+.*#\[(allow|expect)'` prints nothing, except
the `#[cfg(test)]`-only fault point if the atomicity test needs one, which
carries no lint attribute). A Clippy diagnosis in a path that is neither a
writePath nor a declared sharedPath is stop condition 1.

#### Base and review range

- Playback starts from `10c3eab`. Its unowned-path check compares against
  `10c3eab`.
- Review range: `git diff 1ac457f <playback-accepted> -- <playback writePaths
  and edited sharedPaths>`.
- SONG-16 starts from `<playback-accepted>`.
- Receipts record `session: 260`, `baseCommit`, `fixes[]` (fix 1 and fix 2 as
  class `production-defect`), and a fingerprint that differs from every earlier
  receipt.
- The cohort count rises only by declared new files that are actually
  created. Each receipt lists them.

### Session 261 resume amendments (2026-10-04)

Session 260 stopped with SONG-ISSUED-PLAYBACK implemented. One strict-Clippy
`dead_code` diagnostic on `bind_issued_owner` blocked it. The operator repair
in `cbfe20c` added `#[cfg(test)]` to that wrapper (no `allow` or `expect`) and
ran rustfmt on `src/sched/song/pools.rs`. The operator then ran the gates
independently: strict Clippy exit 0, and full nextest with 2,800 passed,
0 failed and 3 skipped. The wasm32 host-wasm build and rustfmt `--check` also
passed. SONG-ROUTE8, SONG-ISSUED-RESOLUTION, SONG-STRUCTURAL-CLOCK and
SONG-SHARED-WORK stay accepted and are not redispatched.

The design above and the session 251-260 amendments stay the baseline. The
session 260 amendment remains the contract for playback and SONG-16. This
amendment adds only the operator-repair boundary, the session 261 evidence
layout and the review checkpoints. Where it conflicts with an earlier
amendment, this one wins. The manifest gets a `resumeSession261` entry in place
and is never duplicated. Order is strict serial with `maxConcurrency: 1`:
SONG-ISSUED-PLAYBACK (gate rerun, receipt, review, acceptance), then SONG-16.

#### Operator repair boundary (`bind_issued_owner`)

- `src/song/snapshot/occupancy/lookup/authority.rs` was already a playback
  sharedPath, but only for the resolution-defect purpose. Its
  `sharedPathNotes` purpose now also covers, retroactively, the one
  `#[cfg(test)]` line at `:400` added in `cbfe20c`. That line is recorded in the
  receipt `fixes[]` as class `join-repair` with commit `cbfe20c`. It replaces
  the session 260 `verification-blocker` entry for TASK-105.
- **Why gating hides no production path.** `bind_issued_owner`
  (`authority.rs:401-410`) only maps `Ok(None)` from
  `bind_issued_owner_if_matching` (`:412`) to the refusal `required issued
  execution is not retained at site`. Its only callers are the tests in
  `src/song/routing/prepared.rs:594-660`. Production owner binding runs through
  `src/song/routing/nested/issued/members.rs:301` -> `bind_issued_index`
  (`authority.rs:580`) -> `bind_issued_owner_if_matching` (`:588`). That path
  performs the same selection, membership, owner, placement, seed and entry
  checks.
- **Review check.** Every production caller of the issued owner-binding helpers
  reaches `bind_issued_owner_if_matching`. No production path builds a
  `RetainedOwnerAddress` any other way. `rg -n "bind_issued_owner\b" src`
  lists only `authority.rs:401` and `prepared.rs` test code. If this does not
  hold, the reviewer records an authority finding against SONG-ISSUED-RESOLUTION
  and applies stop condition 1.

#### Playback evidence for session 261

- **Preserve earlier evidence.** Before any gate runs, copy the current
  `tmp/song-mode-riela/session249-playback-{build.log,intent.json,receipt.json}`
  to `tmp/song-s249/SONG-ISSUED-PLAYBACK/attempt-session260/` with
  `sha256.txt`. Existing directories (`attempt-session259/`, `session259/`,
  `session260/`, `session260-resume/`) are never written to.
- **Gate logs.** Run each gate in "Gates" below once in the foreground on
  `cbfe20c`. Write each log to
  `tmp/song-s249/SONG-ISSUED-PLAYBACK/session261-resume/<gate>.log` (`build`,
  `clippy`, `nextest-full`, `nextest-focused`, `wasm`, `fmt`). This overrides
  the session 260 log path `tmp/song-mode-riela/session249-playback-<gate>.log`.
  Those log paths stay declared writePaths but are not written this session. A
  playback fix made during review means rerunning every gate on the fix commit
  into the same directory, with a `-after-fix` suffix.
- **Receipt.** `tmp/song-mode-riela/session249-playback-receipt.json` is
  rewritten in place after the copy. It records:
  - `session: 261` and `baseCommit: cbfe20c`;
  - `fixes[]`, with TASK-101..104 as recorded and TASK-105 as the join repair
    above;
  - `capacity.busSlots`, measured again in this session from a passing test log
    and not copied: `required` (from `SongRoutePlan.required.bus_slots`,
    checked against `tracks + 1 + sum(reserved_generations)`), `available`
    (song-profile free slots, expected 50), and `engineProfile:
    "song_engine_config"`;
  - the measured route-stage work against the 1,000,000 allowance (SM1);
  - `atomicityTrigger`, which keeps the `#[cfg(test)]` staging fault in
    `src/sched/song/pools.rs`. The receipt must also state why no
    within-capacity fixture reaches real pool exhaustion on a later batch
    event, which the session 260 table requires before a fault point may be
    used;
  - `verification[]`, giving each gate's command, exit status, log path and,
    for nextest, its Summary counts;
  - `evidenceFingerprint`: the sha256 over the session 261 gate logs plus the
    source hashes at the reviewed commit. It must differ from
    `5b9ecddbdecf8224cc6ceafe2d67271a9def3f297a9489b113c4428edd1bb0ea` and from
    every earlier receipt.
- **Plan updates.** Tick TASK-101..105 in
  `impl-plans/active/song-mode-issued-playback.md` only when the cited evidence
  exists. Add one progress-log entry for session 261.
- **No code change is expected.** A review finding is fixed only under the
  session 260 writePaths and sharedPaths rules. Any other path is stop
  condition 1.

#### Playback review checkpoints

The review range is `git diff 1ac457f <playback-accepted> --` over the playback
writePaths and the edited sharedPaths (`authority.rs`). The test-integrity,
adversarial and integration reviews each confirm:

1. **Scheduler consumption.** `src/sched/song.rs` creates one `SharedIndexWork`
   collector, passes it to Ready `query_issued_with_work`, and passes the same
   collector to `resolve_issued` for every event (session 260 TASK-003
   evidence, test `one_collector_spans_query_and_every_resolution`).
2. **Atomic pools.** `pools.rs` stages projections and commands. Pools, pending
   queue, cursor and receipts are committed only when the whole batch succeeds,
   and on failure they equal their pre-`realize` values. The fault field and
   its setter exist only under `#[cfg(test)]` and carry no lint attribute.
3. **Fix scope.** `git diff 10c3eab -- src/pattern/eval/song_clock.rs` touches
   only the body of `source_owns_owner`. `src/song/export.rs` uses
   `song_engine_config`, with no `bus_slots = 32` override.
4. **Owner binding.** The operator repair boundary above holds.
5. **Hygiene.** `git diff 10c3eab | grep -E '^\+.*#\[(allow|expect)'` prints
   nothing. The three unowned paths equal `HEAD`. Every touched Rust file is
   below 1000 lines.

Acceptance is recorded in `resumeSession261` together with the accepted commit.

#### SONG-16 (unchanged contract, confirmed seams)

The session 260 "SONG-16" section stays as is. SONG-16 starts from the
playback-accepted commit. These repository facts confirm that the declared
seams are enough:

- **CLI path.** `vactr render <file> <out> --sample-rate N` is dispatched in
  `src/cli/args.rs:401`. It then runs `src/cli/render.rs` `main`, which calls
  `evaluate_song_candidate`, `prepare_song` and `export_song`, then prints
  `rendered N frames at R Hz; ...; state Ended`. No cycle argument exists. The
  CLI test parses this line.
- **Requirement coverage.** The `tests/song_issued_transport.rs` `PROGRAM`
  that becomes `examples/song-mode/requirement-song.vact` contains:
  - reusable Part functions: `make`, `make-slice`, `remove-one`, `edit` and
    `indexed`;
  - composed generators: `chord`, `choose` and `slice` inside Parts;
  - `sequence`, and `part-repeat` with both `:same` and `:vary`;
  - `delete-event` and `overwrite-region`;
  - instrument-selective filtering through `transform-instrument ... lpf`, and
    an effect through `instrument-fx ... :room`.

  The CLI test asserts automatic termination (`state Ended`) and a
  byte-identical second export.
- **New files.** `examples/song-mode/` does not exist yet. Creating
  `requirement-song.vact` creates it, and no other file goes in that
  directory.
- **Evidence.** Final logs go to
  `tmp/song-mode-riela/session249-final-<gate>.log` (none exist yet). Scratch
  logs go to `tmp/song-s249/SONG-16/session261-resume/`. The final receipt
  records the measured capacity need against capacity (31 of 50 expected) under
  the [SM5](../user-qa/pending-song-mode-questions.md#sm5-song-bus-capacity-for-finite-export)
  default (a). It lists the
  [known resolution defect](#known-resolution-defect-not-in-scope) as residual
  risk unless a required test reaches it.
- **Serial join.** SONG-16 updates the plan progress logs, moves completed
  plans to `impl-plans/completed/`, and updates `impl-plans/README.md`.

### Session 261 final evidence checkpoint (2026-10-04)

- The accepted static-song source is `examples/song-mode/requirement-song.vact`; its SHA-256 is `1dc59ef3ed9edb771b0f85a04262ddde690d5d981d76772258024367269e7e7a`, matching the extracted former `PROGRAM` bytes.
- `tests/song_requirement_cli.rs` verifies production CLI rendering to Ended, duration-derived frame count, WAV size/nonzero audio and byte-identical repeat output, plus the route-work threshold.
- Final build, strict all-target clippy, focused nextest (119/119 across eight binaries), full nextest (2,802 passed, zero failed, three skipped), WASM host build, scoped rustfmt check, editor tests (590/590) and editor build exit 0.
- Full gate logs: `tmp/song-mode-riela/session249-final-{build,clippy,nextest-focused,nextest-full,wasm,fmt,editor-test,editor-build}.log`; route-work evidence: `tmp/song-s249/SONG-16/session261-resume/route-work.log`.
- Rust cohort: 954 accepted baseline files plus the one declared new `tests/song_requirement_cli.rs`; total projection is 957 entries including `Cargo.toml` and `Cargo.lock`. Changed Rust paths are only `tests/song_issued_transport.rs` and that new test; unowned changes are empty.
- The SONG-ISSUED-PLAYBACK capacity receipt records 31 required bus slots against 50 available. Route-work binary search measured W=941,965, below the 1,000,000 allowance.
- SM1 keeps truthful refusal above the default preparation allowance; SM2 keeps pre-Reserve retention without runtime first execution; SM5 keeps truthful capacity refusal under default (a).
- SONG-16 requirement tests and all final gates pass on the session-261 working tree. The known out-of-scope resolution defect remains recorded as residual risk for larger fixture variants.
- Evidence and final receipt: `tmp/song-mode-riela/session249-final-receipt.json`. Six completed plans are archived under `impl-plans/completed/`; the six README entries point there.
