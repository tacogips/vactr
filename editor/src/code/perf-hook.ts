import type { Client } from '../protocol/client';
import type { Store } from '../protocol/store';
import type { Provenance } from '../app/clock';
import type { CodeSurface } from './surface';
import type { HighlightScheduler } from './highlight';
import type { CodeRenderer, RendererKind } from './renderer-types';
import type { ResourceLedger } from './resources';
import { PERF_PHASES, type PerfPhase, type PerfRecorder } from './frame';
import type { TransportSample } from '../protocol/types';

export interface PresentedRecord {
  frameMs: number; targetMs: number; audibleTime: number; provenance: Provenance; valid: boolean;
  activeKey: string; beatCycle: number | null; beatFlash: boolean; revision: number; handles: number;
  executionMs?: number; epoch?: string | null;
}
export interface OnsetRecord { time: number; end: number; from: number; to: number; epoch: string | null; receivedMs: number }
export interface VactrPerf {
  perf: PerfRecorder;
  phases(): { names: PerfPhase[]; rows: number[][] };
  revision(): number;
  ledger: ResourceLedger;
  doc(): string;
  selection(): { anchor: number; head: number };
  presented(): PresentedRecord[];
  onsets(): OnsetRecord[];
  transportSample(): TransportSample | null;
  counters(): { highlight: HighlightScheduler['stats']; probe: null; client: Client['queueStats']; syntaxTruncated: number;
    textPending: boolean; renderer: CodeRenderer['stats']; rendererKind: RendererKind; domNodes: number;
    gpuStatus: { kind: string; effectiveDpr: number }; layout: CodeRenderer['layout']['stats'];
    ledger: ResourceLedger['counters']; usedBytes: number };
  disposeCode(): void;
}

interface PerfInputs {
  win: Window; perf: PerfRecorder; surface: CodeSurface; revision(): number; ledger: ResourceLedger;
  highlight: HighlightScheduler; renderer: CodeRenderer; rendererKind: RendererKind; client: Client; store: Store;
  syntaxTruncated(): number; disposeCode(): void;
}

export function installPerfHook(input: PerfInputs): VactrPerf {
  const presentedRows: PresentedRecord[] = new Array(4096);
  const onsetRows: OnsetRecord[] = new Array(4096);
  let presentedNext = 0, presentedCount = 0, onsetNext = 0, onsetCount = 0;
  const ordered = <T>(rows: T[], next: number, count: number): T[] => {
    const out: T[] = []; const start = (next - count + rows.length) % rows.length;
    for (let i = 0; i < count; i++) { const value = rows[(start + i) % rows.length]; if (value !== undefined) out.push(value); }
    return out;
  };
  const api: VactrPerf = {
    perf: input.perf, phases: () => ({ names: [...PERF_PHASES], rows: input.perf.phases.rows() }),
    revision: input.revision, ledger: input.ledger,
    doc: () => input.surface.state.doc.toString(),
    selection: () => ({ anchor: input.surface.state.selection.main.anchor, head: input.surface.state.selection.main.head }),
    presented: () => ordered(presentedRows, presentedNext, presentedCount),
    onsets: () => ordered(onsetRows, onsetNext, onsetCount),
    transportSample: () => input.store.transportSample ?? null,
    counters: () => ({ highlight: input.highlight.stats, probe: null, client: input.client.queueStats,
      syntaxTruncated: input.syntaxTruncated(), textPending: input.renderer.textPending, renderer: input.renderer.stats,
      rendererKind: input.rendererKind, domNodes: input.win.document.getElementsByTagName('*').length,
      gpuStatus: { kind: input.renderer.status.kind, effectiveDpr: input.renderer.status.effectiveDpr },
      layout: input.renderer.layout.stats, ledger: input.ledger.counters, usedBytes: input.ledger.usedBytes }),
    disposeCode: input.disposeCode,
  };
  Object.defineProperty(api, 'recordPresented', { value: (record: PresentedRecord) => {
    presentedRows[presentedNext] = record; presentedNext = (presentedNext + 1) % presentedRows.length;
    presentedCount = Math.min(presentedRows.length, presentedCount + 1);
  } });
  Object.defineProperty(api, 'recordOnset', { value: (record: OnsetRecord) => {
    onsetRows[onsetNext] = record; onsetNext = (onsetNext + 1) % onsetRows.length;
    onsetCount = Math.min(onsetRows.length, onsetCount + 1);
  } });
  (input.win as Window & { __vactrPerf?: VactrPerf }).__vactrPerf = api;
  return api;
}

export function recordPresented(api: VactrPerf | null, record: PresentedRecord): void {
  (api as (VactrPerf & { recordPresented?: (value: PresentedRecord) => void }) | null)?.recordPresented?.({
    ...record,
    executionMs: performance.now(),
  });
}
export function recordOnset(api: VactrPerf | null, record: OnsetRecord): void {
  (api as (VactrPerf & { recordOnset?: (value: OnsetRecord) => void }) | null)?.recordOnset?.(record);
}
export function removePerfHook(win: Window): void { delete (win as Window & { __vactrPerf?: VactrPerf }).__vactrPerf; }
