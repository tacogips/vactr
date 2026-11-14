import { afterEach, describe, expect, it } from 'vitest';
import { ResourceLedger } from '../../src/code/resources';
import { BackgroundOverlays } from '../../src/visual/overlays';
import { installCanvasFakes, type CanvasFakes } from '../support/canvas';

let fakes: CanvasFakes;
afterEach(() => { fakes?.restore(); document.body.innerHTML = ''; });
function setup() {
  fakes = installCanvasFakes();
  const parent = document.createElement('div'); document.body.append(parent);
  const ledger = new ResourceLedger(); const statuses: string[] = [];
  const overlays = new BackgroundOverlays(parent, ledger, (status) => statuses.push(status));
  return { parent, ledger, statuses, overlays };
}
const settings = (kind: 'oscilloscope' | 'spectrum' | 'spectrogram' | 'level') => ({ kind, opacity: 0.6, position: 'bottom' as const });

describe('master background overlays', () => {
  it('draws the actual waveform and hides scope for unavailable/stale data while retaining independent bands', () => {
    const { parent, overlays, statuses, ledger } = setup();
    overlays.configure([settings('oscilloscope'), settings('spectrum')]);
    const scope = parent.querySelector<HTMLCanvasElement>('[data-kind="oscilloscope"]')!;
    const bands = parent.querySelector<HTMLCanvasElement>('[data-kind="spectrum"]')!;
    overlays.receive({ time: 1, levels: [{ source: ':master', rms: 0.2, bands: [0.1, 0.3] }] }, 1);
    overlays.present(1);
    expect(scope.hidden).toBe(true); expect(bands.hidden).toBe(false);
    expect(statuses.at(-1)).toContain('oscilloscope');
    const waveform = Array.from({ length: 256 }, (_, index) => index === 0 ? 1 : -1);
    overlays.receive({ time: 2, levels: [{ source: ':master', rms: 0.2, bands: [0.1, 0.3], waveform }] }, 2);
    overlays.present(2);
    expect(scope.hidden).toBe(false);
    expect(fakes.ctx(scope).named('moveTo').at(-1)?.args).toEqual([0, 0]);
    expect(fakes.ctx(scope).named('lineTo').at(-1)?.args).toEqual([1024, 256]);
    expect(statuses.at(-1)).toBe('');
    overlays.present(5);
    expect(scope.hidden).toBe(true); expect(bands.hidden).toBe(true);
    overlays.dispose(); expect(ledger.usedBytes).toBe(0); expect(parent.children).toHaveLength(0);
  });

  it('does not revive cached data on configuration and expires native receipt-time data', () => {
    const { parent, overlays } = setup();
    overlays.receive({ levels: [{ source: ':master', rms: 0.2, waveform: Array(256).fill(0.1) }] }, 1);
    overlays.configure([settings('oscilloscope')]);
    const canvas = parent.querySelector('canvas')!;
    expect(canvas.hidden).toBe(true);
    overlays.present(1, true); expect(canvas.hidden).toBe(false);
    overlays.present(4, true); expect(canvas.hidden).toBe(true);
    overlays.dispose();
  });

  it('bounds rolling history and skips repainting unchanged telemetry', () => {
    const { parent, overlays } = setup(); overlays.configure([settings('spectrogram')]);
    const canvas = parent.querySelector('canvas')!;
    for (let i = 0; i < 110; i++) {
      overlays.receive({ time: i, levels: [{ source: ':master', rms: 0.1, bands: Array(8).fill(0.5) }] }, i);
      fakes.ctx(canvas).reset(); overlays.present(i);
    }
    expect(fakes.ctx(canvas).named('fillRect')).toHaveLength(96 * 8);
    const paints = fakes.ctx(canvas).calls.length; overlays.present(109);
    expect(fakes.ctx(canvas).calls).toHaveLength(paints);
    overlays.dispose();
  });
});
