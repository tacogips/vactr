import type { Accessor, JSX } from 'solid-js';

export function CanvasDisplayView(props: { title: string; kind: string; width: number; height: number }): JSX.Element {
  return <div class="visual-display" data-kind={props.kind}>
    <span class="visual-display-title">{props.title}</span>
    <canvas width={props.width} height={props.height} />
  </div>;
}

export function ReadoutDisplayView(props: { title: string; kind: string; text: Accessor<string> }): JSX.Element {
  return <div class="visual-display" data-kind={props.kind}>
    <span class="visual-display-title">{props.title}</span>
    <span class="visual-readout">{props.text()}</span>
  </div>;
}

export function AnalyzerView(): JSX.Element {
  return <section class="visual-analyzers" data-area="analyzers">
    <div class="visual-master" />
    <div class="visual-analyzer-list" />
  </section>;
}
