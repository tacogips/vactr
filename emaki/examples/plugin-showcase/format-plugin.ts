import type { EmakiPlugin, PluginHost, Rgba } from '../../src/index';

const accent: Rgba = [0.44, 0.71, 1, 1];
const warm: Rgba = [1, 0.68, 0.35, 1];

export function formatPlugin(): EmakiPlugin {
  return { name: 'formats', attach(host: PluginHost) {
    const line = (n: number) => host.state.doc.line(Math.min(n, host.state.doc.lines));
    const word = (n: number) => { const l = line(n); return { from: l.from, to: Math.min(l.to, l.from + 5) }; };
    const update = () => {
      host.marks('static').set([
        { ...word(1), style: { fontWeight: 'bold' } },
        { ...word(2), style: { underline: { color: accent, thickness: 2 } } },
        { ...word(3), style: { strike: { color: warm } } },
        { ...word(4), style: { color: accent } },
      ]);
      host.lineFormats('lines').set([
        { pos: line(1).from, style: { bar: { color: accent, width: 3 } } },
        { pos: line(2).from, style: { background: [0.12, 0.22, 0.3, 0.8] } },
        { pos: line(3).from, style: { gutterMarker: { text: '*', color: warm } } },
      ]);
      host.widgets('badge').set([], [{ key: 'badge', pos: line(4).from, width: 48,
        content: { kind: 'canvas', draw(ctx, info) {
          ctx.fillStyle = '#28465d'; ctx.fillRect(0, 0, info.width, info.height);
          ctx.fillStyle = '#e6ebf2'; ctx.font = '11px sans-serif'; ctx.fillText('LIVE', 7, info.height - 4); return false;
        } } }]);
    };
    update();
    const dispose = host.animatedMarks(ms => {
      const l = line(4); if (l.length === 0) return [];
      const from = l.from + Math.floor(ms / 500) % l.length;
      return [{ from, to: from + 1, style: { glow: { color: accent, radius: 5 } } }];
    });
    host.setActive(true);
    return { docChanged: update, destroy: dispose };
  } };
}
