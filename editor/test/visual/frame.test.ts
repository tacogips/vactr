// The frame loop (design 9.3, 15.1.8): `core.frame` before `draw` on every
// animation frame at the clock's time; pause while hidden; stop on dispose.

import { describe, expect, it } from 'vitest';

import { startFrameLoop, type FrameScheduler } from '../../src/visual/frame';
import { MockClock } from '../support/clock';

class FakeRaf implements FrameScheduler {
  private next = 1;
  readonly pending = new Map<number, () => void>();

  request(cb: () => void): number {
    const id = this.next++;
    this.pending.set(id, cb);
    return id;
  }

  cancel(id: number): void {
    this.pending.delete(id);
  }

  /** Runs the callbacks pending now (one animation frame). */
  step(): void {
    const due = [...this.pending.values()];
    this.pending.clear();
    for (const cb of due) cb();
  }
}

function rig() {
  const log: string[] = [];
  const clock = new MockClock(1);
  const raf = new FakeRaf();
  const core = { frame: (now: number) => void log.push(`frame:${now}`) };
  const host = { draw: (t: number) => void log.push(`draw:${t}`) };
  const loop = startFrameLoop({ core, host, clock, scheduler: raf, onFrame: () => void log.push('present') });
  return { log, clock, raf, loop };
}

describe('startFrameLoop', () => {
  it('calls core.frame before draw on each frame', () => {
    const { log, clock, raf, loop } = rig();
    expect(loop.running).toBe(true);
    raf.step();
    clock.advance(0.5);
    raf.step();
    expect(log).toEqual(['frame:1', 'draw:1', 'present', 'frame:1.5', 'draw:1.5', 'present']);
    expect(raf.pending.size).toBe(1);
  });

  it('stops on dispose', () => {
    const { log, raf, loop } = rig();
    raf.step();
    loop.dispose();
    expect(raf.pending.size).toBe(0);
    expect(loop.running).toBe(false);
    raf.step();
    loop.setVisible(true);
    raf.step();
    expect(log).toEqual(['frame:1', 'draw:1', 'present']);
  });

  it('pauses while hidden and resumes when visible', () => {
    const { log, raf, loop } = rig();
    loop.setVisible(false);
    expect(raf.pending.size).toBe(0);
    raf.step();
    expect(log).toEqual([]);
    loop.setVisible(true);
    raf.step();
    expect(log).toEqual(['frame:1', 'draw:1', 'present']);
  });

  it('stops and reports when a frame throws', () => {
    const raf = new FakeRaf();
    const errors: unknown[] = [];
    let draws = 0;
    startFrameLoop({
      core: {
        frame: () => {
          throw new Error('wasm trap');
        },
      },
      host: { draw: () => void draws++ },
      clock: new MockClock(),
      scheduler: raf,
      onError: (e) => errors.push(e),
    });
    raf.step();
    raf.step();
    expect(draws).toBe(0);
    expect(errors).toHaveLength(1);
    expect(raf.pending.size).toBe(0);
  });
});
