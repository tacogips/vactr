import type { Text } from '@codemirror/state';
import type { LayoutViewport, TextLayout } from '../layout/layout';
import type { PhaseTimer } from './frame';
import type { Palette } from './palette';
import type { AnimatedMark, CursorShape, Mark, Range, ResolvedLineFormat, TokenSpan } from '../types';
export interface GpuStatus { kind: 'ready' | 'degraded' | 'unavailable' | 'context-lost'; message: string; effectiveDpr: number; saveText(): string }
export interface RenderInput {
  textRevision?: number; decorationsKey?: string; tokens?: readonly TokenSpan[];
  selection?: readonly Range[]; composition?: readonly Range[]; marks?: readonly Mark[]; animatedMarks?: readonly AnimatedMark[];
  lineFormats?: readonly ResolvedLineFormat[]; cursor?: number | null; cursorShape?: CursorShape; cursorEnd?: number | null; cursorVisible?: boolean;
  handles?: readonly { pos: number; end?: boolean }[]; hopLabels?: readonly { pos: number; text: string }[]; hopRevision?: number;
}
export interface CodeRenderer {
  readonly layout: TextLayout; readonly stats: Readonly<Record<string, number>>; readonly status: GpuStatus; readonly textPending: boolean;
  setText(doc: Text): void; setViewport(view: LayoutViewport, dpr?: number): void; render(input?: RenderInput): boolean;
  setPalette(palette: Palette): void; setBackgroundOpacity(opacity: number): void; setPhases(phases: PhaseTimer | null): void; dispose(): void;
}
