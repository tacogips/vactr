import type { Requirement } from '../documents/types';

export interface ManifestEntry {
  id: string;
  title: string;
  description: string;
  category: string;
  requires?: readonly Requirement[];
}

export const DEFAULT_EXAMPLE_ID = 'showcase';

export const CURATED: readonly ManifestEntry[] = Object.freeze([
  { id: 'showcase', title: 'Showcase', description: 'A sample-free techno showcase with a live visual chain.', category: 'Showcase' },
  { id: 'canon-in-d', title: 'Canon In D', description: "Canon in D: Johann Pachelbel. The eight-chord ground bass loops under three of the canon's entries, one per two cycles. The sounds are new", category: 'Examples', },
  { id: 'digital-kit', title: 'Digital Kit', description: "digital-kit: the original programmable digital drum kit's four voice families, reached through the prelude's `digital-kit` alias dict: `bd`", category: 'Examples', },
  { id: 'filter-lab', title: 'Filter Lab', description: 'See examples/effect-labs/ for each independently runnable variant. Sample-free effect audition. Persistent bus keeps tails between triggers', category: 'Examples', },
  { id: 'first-track', title: 'First Track', description: 'first track: a house sketch at 124 bpm, synths only', category: 'Examples', },
  { id: 'fm6-algorithms', title: 'Fm6 Algorithms', description: 'Algorithm 0 keeps the legacy two-operator fm voice; 1 through 32 select six-operator topologies. Ratio, index and velocity shape the macro o', category: 'Examples', },
  { id: 'industrial-techno-bass-rumble', title: 'Industrial Techno Bass Rumble', description: 'Bass-forward industrial techno preset: Peaks FM drum with Mutable Instruments voice adaptations. First four bars expose the bass; kick and k', category: 'Examples', },
  { id: 'industrial-techno', title: 'Industrial Techno', description: 'Industrial techno sketch: Peaks FM drum with Mutable Instruments voice adaptations. Eight parts: dry kick, backbeat, metal hits, ducked bass', category: 'Examples', },
  { id: 'live-external-voices', title: 'Live External Voices', description: 'Opt-in instrument-local host input. Requires a connected stereo input; disconnected lanes are zero. Rings sums both lanes at half gain for m', category: 'Examples', requires: ['audio-input'], },
  { id: 'lofi-lab', title: 'Lofi Lab', description: 'Lo-fi bus demo: alternating four-bar clean and degraded statements', category: 'Examples', },
  { id: 'microtonal-tuning', title: 'Microtonal Tuning', description: 'Microtonal tuning examples using the built-in synth voices', category: 'Examples', },
  { id: 'musicdsp-synth-lab', title: 'Musicdsp Synth Lab', description: 'MusicDSP synthesis sketch: DSF, independent Chebyshev harmonics, AM formants, normal noise. Standalone Lorenz/Rossler and each other kernel', category: 'Examples', },
  { id: 'ode-to-joy', title: 'Ode To Joy', description: "Ode to Joy: the main theme of Beethoven's Symphony No. 9 in C major, one bar per cycle. The arrangement is new. How the notes are written", category: 'Examples', },
  { id: 'resonator-lab', title: 'Resonator Lab', description: 'See examples/effect-labs/ for each independently runnable variant. Sample-free effect audition. Persistent bus keeps tails between triggers', category: 'Examples', },
  { id: 'reverb-delay-lab', title: 'Reverb Delay Lab', description: 'See examples/effect-labs/ for each independently runnable variant. Sample-free effect audition. Persistent bus keeps tails between triggers', category: 'Examples', },
  { id: 'rumble-kick', title: 'Rumble Kick', description: 'Rumble kick: the same Peaks-style FM kick pattern feeds a dry kick bus and a persistent, fully wet reverb bus. The actual dry kick audio key', category: 'Examples', },
  { id: 'stage-linked', title: 'Stage Linked', description: 'Opt-in immutable 1–36 segment chain adaptation. Each row is [type loop primary secondary]: type 0 ramp, 1 step, 2 hold, 3 oscillator. Load t', category: 'Examples', },
  { id: 'strum-harp', title: 'Strum Harp', description: 'Strum, harp and chord performance patterns on existing synth voices', category: 'Examples', },
  { id: 'techno-pattern', title: 'Techno Pattern', description: 'Techno pattern: Peaks FM drum with Mutable Instruments voice adaptations. Eight parts: kick, backbeat, closed hats, bass, stabs, texture, ru', category: 'Examples', },
  { id: 'voice-layer', title: 'Voice Layer', description: 'Opt-in Plaits voice-layer controls on one FM pair voice', category: 'Examples', },
  { id: 'synth-labs/am-formant', title: 'Am Formant', description: 'Original MusicDSP-inspired am-formant synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'synth-labs/chebyshev', title: 'Chebyshev', description: 'Original MusicDSP-inspired chebyshev synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'synth-labs/dsf', title: 'Dsf', description: 'Original MusicDSP-inspired dsf synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'synth-labs/gaussian', title: 'Gaussian', description: 'Original MusicDSP-inspired gaussian synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'synth-labs/lorenz', title: 'Lorenz', description: 'Original MusicDSP-inspired lorenz synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'synth-labs/rossler', title: 'Rossler', description: 'Original MusicDSP-inspired rossler synthesis, sample-free. Lorenz/Rossler are chaotic textures; note values do not imply tuned pitch', category: 'Synth labs', },
  { id: 'effect-labs/alien-wah', title: 'Alien Wah', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/allpass-filter', title: 'Allpass Filter', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/butterworth-filter', title: 'Butterworth Filter', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/chebyshev-filter', title: 'Chebyshev Filter', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/comb-allpass', title: 'Comb Allpass', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/comb-feedback', title: 'Comb Feedback', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/comb-feedforward', title: 'Comb Feedforward', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/diffusion-delay', title: 'Diffusion Delay', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/dynamic-convolution', title: 'Dynamic Convolution', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/early-reflections', title: 'Early Reflections', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/feedback-resonator', title: 'Feedback Resonator', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/foldback', title: 'Foldback', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/ladder-filter', title: 'Ladder Filter', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/parametric-resonator', title: 'Parametric Resonator', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/schroeder-reverb', title: 'Schroeder Reverb', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/shimmer-reverb', title: 'Shimmer Reverb', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/space-reverb', title: 'Space Reverb', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/spring-reverb', title: 'Spring Reverb', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/svf-filter', title: 'Svf Filter', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/tape-delay', title: 'Tape Delay', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
  { id: 'effect-labs/variable-clip', title: 'Variable Clip', description: 'Sample-free effect audition. Persistent bus keeps tails between triggers. Left = quiet dry reference; right = effect. Set both pans to 0.5 f', category: 'Effect labs', },
]);

export const EXCLUDED: readonly { path: string; reason: string }[] = Object.freeze([
  { path: 'sample-timestamps.vact', reason: 'needs ./samples/amen-recreation/source.wav, which is not bundled' },
  { path: 'tracks/breakcore/glass-teeth.vact', reason: 'needs the examples/samples/amen-recreation WAV, which is not bundled' },
  { path: 'quad-stems.vact', reason: 'needs a four-output native host' },
  { path: 'acid-techno.vact', reason: 'browser-tier evaluation fails: `:phase-drum` is not a sound in the default sound kit' },
  { path: 'bass-presets.vact', reason: 'browser-tier evaluation fails: `:analog-bass` is not a sound in the default sound kit' },
  { path: 'experimental-oscillators.vact', reason: 'browser-tier evaluation fails: `:vosim` is not a sound in the default sound kit' },
  { path: 'hurdy-gurdy.vact', reason: 'browser-tier evaluation fails: `:hurdy-gurdy` is not a sound in the default sound kit' },
  { path: 'kalimba.vact', reason: 'browser-tier evaluation fails: `:kalimba` is not a sound in the default sound kit' },
  { path: 'offbeat-bass-techno.vact', reason: 'browser-tier evaluation fails: `:phase-drum` is not a sound in the default sound kit' },
  { path: 'rumble-techno.vact', reason: 'browser-tier evaluation fails: `:phase-drum` is not a sound in the default sound kit' },
  { path: 'tonewheel-organ.vact', reason: 'browser-tier evaluation fails: `:tonewheel-organ` is not a sound in the default sound kit' },
  { path: 'wobble-techno.vact', reason: 'browser-tier evaluation fails: `:phase-drum` is not a sound in the default sound kit' },
  { path: 'song-mode/generated-parts.vact', reason: 'browser-tier evaluation fails: finite song playback is unavailable until song transport is installed' },
  { path: 'song-mode/requirement-song.vact', reason: 'browser-tier evaluation fails: finite song playback is unavailable until song transport is installed' },
  { path: 'song-mode.vact', reason: 'browser-tier evaluation fails: finite song playback is unavailable until song transport is installed' },
]);

export const TRACK_CATEGORY_PREFIX = 'Tracks: ';
