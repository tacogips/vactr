// Cross-plan area API contracts (design 15.1.3 layout, 15.1.5-15.1.9).
// Each area's `mount` sets its API on `EditorDeps`; a consumer reads
// `deps.<area>` at USE time and treats a missing API as absent (for
// example `midi` on a tier without WebMIDI). CE-JOIN finalizes the canvas cutover contract.

import type { EditorState, Transaction, TransactionSpec, ChangeSet } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { Span, WireSite } from '../protocol/types';

export interface SampleFrames {
  /** Interleaved frames. */
  data: Float32Array;
  rate: number;
  channels: number;
}

export interface SampleLibraryApi {
  frames(bank: string, index: number): SampleFrames | null;
  openBrowser(bank?: string): void;
}

/** Shared per-instance resource accounting; reserve before allocating replacements. */
export interface ResourceBudget {
  readonly limitBytes: number;
  readonly usedBytes: number;
  reserve(bytes: number): boolean;
  release(bytes: number): void;
}

export interface CodeRange { from: number; to: number }
export interface CodeRect { left: number; right: number; top: number; bottom: number }
export interface CodeAnnotation extends CodeRange {
  kind: 'syntax' | 'selection' | 'playing' | 'eval' | 'diagnostic' | 'binding' | 'composition';
  className?: string;
  label?: string;
}
export interface CodeSurfaceUpdate {
  state: EditorState;
  changes: ChangeSet;
  docChanged: boolean;
  selectionSet: boolean;
}

/** Concrete headless authority. Offsets are UTF-16; coordinates are viewport CSS pixels. */
export interface CodeSurface {
  readonly state: EditorState;
  dispatch(...specs: (TransactionSpec | Transaction)[]): void;
  subscribe(cb: (update: CodeSurfaceUpdate) => void): () => void;
  focus(): void;
  posAtCoords(coords: { x: number; y: number }): number | null;
  coordsAtPos(pos: number): CodeRect | null;
  /** Replace one owner's presentation ranges; never modifies the document. */
  annotate(owner: string, ranges: readonly CodeAnnotation[]): void;
  onPointer(cb: (event: PointerEvent) => void): () => void;
  readonly compositionRange: CodeRange | null;
  /** Defer overlapping writes until composition ends; callback revalidates site/text. */
  deferSourceWrite(range: CodeRange, write: () => void): void;
}

export interface CodeApi {
  /** Supplied by CE-STATE; CE-JOIN makes this required and removes view. */
  surface?: CodeSurface;
  view: EditorView;
  /** A wire span of revision `rev` mapped to the current UTF-16 range, or null when gone. */
  mapWireSpan(span: Span, rev: number): { from: number; to: number } | null;
  currentRevision(file: string): number;
  selectedSiteId(): number | null;
  samples: SampleLibraryApi;
}

export interface MidiCcEvent {
  cc: number;
  ch: number;
  value: number;
  time: number;
}

export interface MidiApi {
  onCc(cb: (ev: MidiCcEvent) => void): () => void;
  learnNext(): Promise<{ cc: number; ch: number }>;
  cancelLearn(): void;
}

export interface VisualApi {
  /** Context-local canvas copy source; subscribers release their listener on disposal. */
  onBackgroundCanvas?(cb: (canvas: HTMLCanvasElement | null) => void): () => void;
  budget?: ResourceBudget;
  mountSpectrum(el: HTMLElement, source: { bus?: string }): { dispose(): void };
}

export type SiteMode = 'overlay' | 'source-edit';

export interface BindApi {
  writeSite(siteId: number, value: number): void;
  mode(siteId: number): SiteMode;
  learn(siteId: number): Promise<void>;
  siteById(id: number): WireSite | undefined;
}
