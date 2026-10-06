// Read-only Solid views of scheduler telemetry. No write path is exposed.
import { For, type Accessor, type JSX } from 'solid-js';
import type { GridStep } from './grid';
import type { RollNote } from './roll';

export interface GridRow {
  slot: string;
  steps: GridStep[];
}

export function GridView(props: { rows: Accessor<GridRow[]> }): JSX.Element {
  return (
    <div class="params-grid">
      <For each={props.rows()}>{(row) =>
        <div class="params-grid-row" data-slot={row.slot}>
          <span class="params-grid-slot">{row.slot}</span>
          <For each={row.steps}>{(step) =>
            <span class="params-grid-step" data-pos={String(Number(step.pos.toFixed(6)))}
              style={{ left: `${step.pos * 100}%`, width: `${Math.max(0.5, step.len * 100)}%` }} />
          }</For>
        </div>
      }</For>
    </div>
  );
}

export interface RollDisplayNote extends RollNote {
  lane: string | undefined;
}

export function RollView(props: { notes: Accessor<RollDisplayNote[]>; range: Accessor<{ hi: number; lanes: number }> }): JSX.Element {
  return (
    <div class="params-roll">
      <For each={props.notes()}>{(note) =>
        <span class="params-roll-note" data-slot={note.slot} data-lane={note.lane}
          data-pitch={note.pitch === null ? undefined : String(note.pitch)}
          style={{ left: `${note.pos * 100}%`, width: `${Math.max(0.5, note.len * 100)}%`,
            top: `${((note.pitch === null ? props.range().lanes - 1 : props.range().hi - note.pitch) / props.range().lanes) * 100}%` }}
          title={note.text} />
      }</For>
    </div>
  );
}
