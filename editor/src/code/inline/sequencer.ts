import type { BlockWidgetSpec, EmakiPlugin, PluginHost } from 'emaki';
import type { Span, WirePlaying } from '../../protocol/types';
import { CycleSteps } from '../../params/grid';
import { slotAnchors } from './anchors';

export interface SequencerDeps {
  file: string; beatsPerCycle(): number; mapSpan(span: Span, rev: number): { from: number; to: number } | null;
  forms(): readonly { from: number; to: number }[]; cycles(): number | null; running(): boolean; enabled(): boolean;
}
export function createSequencerPlugin(deps: SequencerDeps): { plugin: EmakiPlugin; onPlaying(events: readonly WirePlaying[]): void; clear(): void; refresh(): void } {
  const steps = new CycleSteps();
  const firstSrc = new Map<string, { span: Span; rev: number }>();
  const cycles = new Map<string, number>();
  let host: PluginHost | undefined;
  let published = false;
  let colors: { accent: string; muted: string; raised: string } | undefined;
  const refresh = (): void => {
    if (!host) return;
    if (!deps.enabled()) { if (published) host.widgets('seq').clear(); published = false; return; }
    const anchors = slotAnchors({ firstSrc, mapSpan: deps.mapSpan, forms: deps.forms(), doc: host.state.doc });
    const blocks: BlockWidgetSpec[] = [...anchors].map(([line, slots]) => ({
      key: `seq:${line}`, pos: host!.state.doc.line(line + 1).from, side: 'below', height: slots.length * 14 + 6,
      content: { kind: 'canvas', draw(ctx, { width, height }) {
        if (!colors) {
          const css = getComputedStyle(ctx.canvas);
          colors = { accent: css.getPropertyValue('--vt-accent').trim() || '#7fe0a0', muted: css.getPropertyValue('--vt-text-muted').trim() || '#777', raised: css.getPropertyValue('--vt-raised').trim() || '#222' };
        }
        ctx.fillStyle = colors.raised; ctx.fillRect(0, 0, width, height);
        const now = deps.cycles(); const phase = now === null ? -1 : now - Math.floor(now);
        slots.forEach((slot, row) => { for (const step of steps.steps(slot)) {
          ctx.fillStyle = phase >= step.pos && phase < step.pos + step.len ? colors!.accent : colors!.muted;
          ctx.fillRect(step.pos * width, row * 14 + 3, Math.max(2, step.len * width - 1), 10);
        } });
        return deps.running();
      } },
    }));
    if (blocks.length) { host.widgets('seq').set(blocks); published = true; }
    else if (published) { host.widgets('seq').clear(); published = false; }
  };
  return {
    plugin: { name: 'inline-sequencer', attach(h) { host = h; refresh(); return { destroy() { host = undefined; } }; } },
    onPlaying(events) {
      const bpc = deps.beatsPerCycle() > 0 ? deps.beatsPerCycle() : 4;
      steps.add(events, bpc);
      for (const event of events) {
        const beat = event.beat[1] === 0 ? 0 : event.beat[0] / event.beat[1];
        const cycle = Math.floor(beat / bpc); const old = cycles.get(event.slot);
        if (old === undefined || cycle > old) { cycles.set(event.slot, cycle); firstSrc.delete(event.slot); }
        else if (cycle < old) continue;
        if (event.src?.file === deps.file && (!firstSrc.has(event.slot) || event.src.span.start < firstSrc.get(event.slot)!.span.start)) firstSrc.set(event.slot, { span: event.src.span, rev: event.src.doc_revision });
      }
      refresh(); host?.requestFrame();
    },
    clear() { steps.clear(); cycles.clear(); firstSrc.clear(); refresh(); }, refresh,
  };
}
