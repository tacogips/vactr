import type { EmakiPlugin } from '../../src/index';

export function waveWidgetPlugin({ pos, height = 64 }: { pos: number; height?: number }): EmakiPlugin {
  return {
    name: 'showcase-wave',
    attach(host) {
      host.widgets('wave').set([{
        key: 'wave', pos, side: 'below', height, label: 'Animated waveform',
        content: {
          kind: 'canvas',
          draw(ctx, { width, height: drawHeight, frameMs }) {
            ctx.fillStyle = '#161d27'; ctx.fillRect(0, 0, width, drawHeight);
            ctx.strokeStyle = '#61d6c4'; ctx.lineWidth = 2; ctx.beginPath();
            for (let x = 0; x <= width; x += 2) {
              const y = drawHeight / 2 + Math.sin(x * 0.04 + frameMs * 0.004) * drawHeight * 0.3;
              if (x === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
            }
            ctx.stroke();
            return true;
          },
        },
      }]);
    },
  };
}
