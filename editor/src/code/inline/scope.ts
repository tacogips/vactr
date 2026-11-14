import type { BlockWidgetSpec, EmakiPlugin, PluginHost } from 'emaki';
import type { LevelsBody } from '../../protocol/types';
import { LevelsTimeline } from '../../visual/meters';
import { drawScope } from '../../visual/scopes';
import { masterLevel } from '../../visual/spectrum';
import { scopeAnchors } from './anchors';

export function createScopePlugin(deps: { time(): number; latestOnReceipt(): boolean; enabled(): boolean }): { plugin: EmakiPlugin; onLevels(body: LevelsBody): void; clear(): void; refresh(): void } {
  let timeline = new LevelsTimeline(); let latest: LevelsBody | null = null; let receipt = -Infinity;
  let host: PluginHost | undefined; let pending = false; let lastScan = -Infinity; let anchors: number[] = []; let published = false;
  let idleColor: string | undefined;
  const frameBody = (): LevelsBody | null => deps.latestOnReceipt() ? (performance.now() - receipt <= 2000 ? latest : null) : timeline.at(deps.time());
  const redraw = (): void => {
    if (!host) return;
    if (!deps.enabled() || !anchors.length) { if (published) host.widgets('scope').clear(); published = false; return; }
    const blocks: BlockWidgetSpec[] = anchors.map((line) => ({
      key: `scope:${line}`, pos: host!.state.doc.line(line + 1).from, side: 'below', height: 64,
      content: { kind: 'canvas', draw(ctx, { width, height }) {
        const body = frameBody();
        const wave = body && masterLevel(body)?.waveform;
        if (wave?.length === 256) drawScope(ctx, width, height, wave);
        else {
          idleColor ??= getComputedStyle(ctx.canvas).getPropertyValue('--vt-text-muted').trim() || '#777';
          ctx.fillStyle = idleColor; ctx.fillRect(0, height / 2, width, 1);
        }
        return performance.now() - receipt <= 2000;
      }, version: frameBody },
    }));
    host.widgets('scope').set(blocks);
    published = true;
  };
  const refresh = (): void => {
    if (!host) return;
    pending = false; lastScan = performance.now(); anchors = scopeAnchors(host.state.doc); redraw();
  };
  return {
    plugin: { name: 'inline-scope', attach(h) { host = h; refresh(); return {
      docChanged(update) {
        let relevant = anchors.length > 0;
        if (!relevant) update.changes.iterChangedRanges((_a, _b, from, to) => {
          const doc = update.state.doc;
          for (const line of doc.iterLines(doc.lineAt(from).number, doc.lineAt(to).number + 1)) {
            if (/^\s*#@\s+editor(?:\s|$)/.test(line)) { relevant = true; break; }
          }
        });
        if (relevant) { pending = true; host?.requestFrame(); }
      },
      frame() { if (pending && performance.now() - lastScan >= 250) refresh(); return pending; },
      destroy() { host = undefined; },
    }; } },
    onLevels(body) {
      // Republish only to restart a widget that stopped animating (no levels for 2 s); otherwise the
      // version-gated draw picks the new frame up on the next animation frame.
      const resumed = performance.now() - receipt > 2000;
      receipt = performance.now(); latest = body; timeline.push(body, deps.time());
      if (resumed && !pending) redraw(); else host?.requestFrame();
    },
    clear() { timeline = new LevelsTimeline(); latest = null; receipt = -Infinity; if (!pending) redraw(); }, refresh,
  };
}
