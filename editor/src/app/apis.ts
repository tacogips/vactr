// Cross-plan area API contracts (design 15.1.3 layout, 15.1.5-15.1.9).
// Each area's `mount` sets its API on `EditorDeps`; a consumer reads
// `deps.<area>` at USE time and treats a missing API as absent (for
// example `midi` on a tier without WebMIDI). No later plan edits this file.

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

export interface CodeApi {
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
  mountSpectrum(el: HTMLElement, source: { bus?: string }): { dispose(): void };
}

export type SiteMode = 'overlay' | 'source-edit';

export interface BindApi {
  writeSite(siteId: number, value: number): void;
  mode(siteId: number): SiteMode;
  learn(siteId: number): Promise<void>;
  siteById(id: number): WireSite | undefined;
}
