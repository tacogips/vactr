import { describe, expect, it, vi } from 'vitest';
import { TextLayout } from '../../src/layout/layout';
import { ResourceLedger } from '../../src/render/resources';
import { WidgetLayer } from '../../src/view/widget-layer';
import type { ResolvedBlockWidget } from '../../src/plugin/widgets';
describe('WidgetLayer', () => {
  it('virtualizes with hysteresis, draws only when visible, and releases resources', () => {
    const ctx = { scale: vi.fn(), clearRect: vi.fn() } as unknown as CanvasRenderingContext2D;
    const getContext = vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(ctx);
    const layout = new TextLayout({ font: '', measureText: s => ({ width: s.length * 8 }) }, { font: '16px mono', lineHeight: 20, baseline: 15 });
    layout.setDocument(Array(10000).fill('x').join('\n')); const ledger = new ResourceLedger(); const host = document.createElement('div'); const layer = new WidgetLayer(host, layout, ledger);
    const draw = vi.fn(() => false); const mount = vi.fn(() => vi.fn());
    const blocks: ResolvedBlockWidget[] = [10, 5000, 9990].map((line, i) => ({ id: String(i), key: String(i), pos: 0, line, side: 'below', height: 10, content: i === 0 ? { kind: 'dom', mount } : { kind: 'canvas', draw } }));
    const update = (scrollTop: number) => layer.update({ view: { width: 300, height: 200, scrollTop, scrollLeft: 0 }, dpr: 1, frameMs: 0, blocks, eols: [] });
    update(0); expect(mount).toHaveBeenCalledOnce(); expect(host.querySelectorAll('.emaki-widget')).toHaveLength(1); expect(draw).not.toHaveBeenCalled();
    expect((host.querySelector('.emaki-widget') as HTMLElement).style.pointerEvents).toBe('none');
    update(550); update(0); expect(mount).toHaveBeenCalledOnce();
    update(99800); expect(mount.mock.results[0].value).toHaveBeenCalledOnce(); expect(draw).not.toHaveBeenCalled();
    update(100000); expect(draw).toHaveBeenCalledOnce(); update(100000); expect(draw).toHaveBeenCalledOnce();
    blocks[1] = { ...blocks[1], content: { kind: 'canvas', draw } }; update(100000); expect(draw).toHaveBeenCalledTimes(2);
    layer.dispose(); expect(ledger.usedBytes).toBe(0); getContext.mockRestore();
  });
});
