import type { EmakiPlugin } from '../../src/index';

export function videoWidgetPlugin({ pos, height = 180 }: { pos: number; height?: number }): EmakiPlugin {
  return {
    name: 'showcase-video',
    attach(host) {
      host.widgets('video').set([{
        key: 'video', pos, side: 'below', height, interactive: true, label: 'Demo video',
        content: {
          kind: 'dom',
          mount(el) {
            const canvas = document.createElement('canvas');
            canvas.width = 320; canvas.height = 180;
            const ctx = canvas.getContext('2d');
            let frame = 0;
            let raf = 0;
            const draw = () => {
              if (ctx) {
                ctx.fillStyle = '#161d27'; ctx.fillRect(0, 0, 320, 180);
                ctx.fillStyle = '#61d6c4'; ctx.fillRect((frame * 2) % 320, 70, 64, 40);
              }
              frame++; raf = requestAnimationFrame(draw);
            };
            let stream: MediaStream | undefined;
            if (typeof canvas.captureStream === 'function') {
              stream = canvas.captureStream(30);
              const video = document.createElement('video');
              video.muted = true; video.playsInline = true; video.autoplay = true; video.loop = true;
              video.srcObject = stream;
              video.style.width = '100%'; video.style.height = '100%';
              el.append(video);
            } else {
              canvas.dataset.fallback = 'captureStream';
              canvas.style.width = '100%'; canvas.style.height = '100%';
              el.append(canvas);
            }
            draw();
            return () => {
              cancelAnimationFrame(raf);
              stream?.getTracks().forEach(track => track.stop());
              el.replaceChildren();
            };
          },
        },
      }]);
    },
  };
}
