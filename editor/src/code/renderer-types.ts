import type { Text } from '@codemirror/state';
import type { LayoutViewport, TextLayout } from './layout';
import type { PhaseTimer } from './frame';
import type { Palette } from './palette';
import type { GpuStatus, RenderFeedback } from './renderer';

export interface CodeRenderer {
  readonly layout: TextLayout;
  readonly stats: Readonly<Record<string, number>>;
  readonly status: GpuStatus;
  readonly textPending: boolean;
  setText(doc: Text): void;
  setViewport(view: LayoutViewport, dpr?: number): void;
  render(feedback?: RenderFeedback): boolean;
  setPalette(palette: Palette): void;
  setPhases(phases: PhaseTimer | null): void;
  dispose(): void;
}
