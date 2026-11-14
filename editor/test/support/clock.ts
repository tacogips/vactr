// MockClock: a settable `Clock` for highlight and timing tests.

import type { Clock } from '../../src/app/clock';

export class MockClock implements Clock {
  private t: number;

  constructor(t = 0) {
    this.t = t;
  }

  now(): number {
    return this.t;
  }

  set(t: number): void {
    this.t = t;
  }

  advance(dt: number): void {
    this.t += dt;
  }
}
