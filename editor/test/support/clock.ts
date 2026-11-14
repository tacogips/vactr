// MockClock: a settable `Clock` for highlight and timing tests.

import type { Clock } from '../../src/app/clock';
import type { Timers } from '../../src/protocol/document';

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

/** A deterministic page clock whose seconds can also drive a fake audio context. */
export class SimulatedTime {
  pageMs = 0;
  contextSeconds = 0;
  private nextId = 1;
  private jobs = new Map<number, { at: number; fn: () => void }>();
  readonly timers: Timers = {
    set: (fn, ms) => {
      const id = this.nextId++;
      this.jobs.set(id, { at: this.pageMs + ms, fn });
      return id;
    },
    clear: (handle) => { this.jobs.delete(handle as number); },
  };

  advance(ms: number): void {
    const target = this.pageMs + ms;
    while (true) {
      const due = [...this.jobs.entries()].filter(([, job]) => job.at <= target).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.pageMs = due[1].at;
      this.contextSeconds = this.pageMs / 1000;
      this.jobs.delete(due[0]);
      due[1].fn();
    }
    this.pageMs = target;
    this.contextSeconds = target / 1000;
  }

  getOutputTimestamp(): { contextTime: number; performanceTime: number } {
    return { contextTime: this.contextSeconds, performanceTime: this.pageMs };
  }
}
