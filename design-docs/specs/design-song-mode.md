# Song mode: reusable finite multi-track parts

**Status**: Accepted design — implementation not started
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

Proposed declarations: part tracks duration: ratio -> part; part-repeat part count seed-mode: keyword -> part; sequence [part] -> part; replace-track part keyword pattern -> part; transform-instrument part keyword sound-selector (fn ctl -> ctl) -> part; part-events part keyword ratio ratio -> [event-handle]; delete-event part event-handle -> part; overwrite-region part keyword ratio ratio pattern -> part; instrument-fx part keyword sound-selector keyword -> part; song part bpm: ratio cycle-beats: ratio meter: [int] seed: int tail-seconds: ratio -> song; play-song song -> song.

part forces duration and the track dictionary at construction. Function/thunk values supplied inside tracks are resolved as patterns using the current language conventions. Part transforms return new values; they never bind output slots. Only play-song stages song playback.

replace-track requires an existing track key. part-events returns read-only descriptors including an opaque handle, local whole span, resolved instrument and individual note value. delete-event accepts the handle, not a bare time position or mutable vector index.

transform-instrument invokes its callback once on the selected symbolic stream in a pure construction context; existing pattern operators may build the result. Matching occurs on the frozen source instrument before transformation. Unmatched music survives. The selected result retains track duration and is clipped by the Part boundary.

instrument-fx associates a declared bus-chain template with a selected instrument branch. The template's effect chain is instantiated privately for that branch; this is not a request to route sibling audio through the shared bus of the same name.

```vact
fn drums:
	part [drums: {s [:bd :sd :bd :sd]} hats: {s :hh > euclid 7 8}] duration: 4

fn developed:
	let base drums
	base > transform-instrument :drums :bd {p -> lpf p 900}

let intro drums
let verse developed
let arrangement sequence [{part-repeat intro 2} {part-repeat verse 4}]
song arrangement bpm: 120 cycle-beats: 4 meter: [4 4] seed: 42 tail-seconds: 8 > play-song
```

### exampleNotes

- The example uses tab indentation, existing brace grouping, dictionary pairs, explicit lambdas and subject-first pipes.
- Intro duration is four cycles; two intro repeats plus four verse repeats produce 24 cycles without a CLI cycle argument.
- A nested function may enumerate handles on its current Part revision, delete one selected handle, replace a track or overwrite a region before returning its transformed Part.
- No claim is made that this proposed example currently evaluates.

## Timing and randomness

Part-local time is exact rational cycles beginning at zero. Arrangement time is the sum of child offsets plus local time. Transport host time begins at one acknowledged song-start boundary. Query spans intersect the arrangement and child bounds before being mapped into local time.

For the initial release, Song owns one constant positive rational BPM and beats-per-cycle value, defaulting to 120 and 4. Meter defaults to [4 4]; numerator must be positive and denominator a positive power of two. Meter describes bars in quarter-note beats and does not silently change pattern cycle length.

Tempo and meter changes inside a Song are unsupported in this release and produce explicit diagnostics. Existing live use-bpm/use-cycle remain valid outside song playback. Reject their application to an active song rather than silently changing its duration or export result. MIDI/Link-following clocks and live input sources are likewise unavailable to deterministic song generation.

Cycle-to-seconds conversion uses the frozen Tempo contract. Host frame endpoints are obtained from absolute positions by one documented round-to-nearest, ties-up rule; never accumulate independently rounded child durations. Logical event times remain rational until the host boundary.

Default repeat seed-mode is :same: each repeat queries the child at local zero with the same derived seed and reproduces its random musical decisions. :vary derives a stable seed from the Song root seed and complete sequence/repeat placement path. Distinct placement identities remain distinct even when draws are identical.

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

## Bounds and diagnostics

Reuse checked Ratio64 arithmetic, VM fuel and existing pattern query bounds. Current query constants are depth 256, re-entry 64 and work budget 1,000,000; song canonical realization must honor them instead of bypassing them.

Add a bounded song construction context for recursive function-generated parts and symbolic edit depth. Its limits are fixed in the runtime configuration and advertised with host capabilities; sequence and repeat durations are checked before scheduling. A repeat count fitting u32 does not exempt its result from duration/frame limits.

Preflight tracks, route branches, simultaneously live/retiring generations, cells, samples, graph units and DSP memory against actual native/browser allocations. Do not assume a nominal voice limit also guarantees branch or arena capacity.

Keep per-query output and canonical caches bounded. Work/fuel failures, overflow, missing assets, stale handles, unknown tracks/instruments/buses, unsupported live input, tempo changes, preparation timeout and host rejection identify snapshot, source span and relevant local/global placement.

A complete-song export fails on any music realization or host fault. Live song failure stops new song onsets and publishes Failed while retaining the editor session for correction. Existing generic pattern sibling-fault behavior remains unchanged outside song mode.

Audio callbacks allocate no song data and execute no user VM callbacks. Candidate evaluation, symbolic edits, resource preparation and canonical realization run on the existing control/session side; audio receives bounded prepared commands.

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
