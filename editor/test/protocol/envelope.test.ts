import { describe, expect, it } from 'vitest';
import { decodeClient, decodeServer, MAX_PLAYING_EVENTS } from '../../src/protocol/envelope';
import type { WirePlaying } from '../../src/protocol/types';

const event: WirePlaying = { slot: 'd1', beat: [0, 1], time: 1, dur: [1, 8] };
const frame = (body: Record<string, unknown>): string => JSON.stringify({ v: 1, seq: 1, kind: 'playing', body });
const clientFrame = (kind: string, body: Record<string, unknown>): string =>
  JSON.stringify({ v: 1, seq: 1, kind, body });

describe('playing envelope announcements', () => {
  it('accepts valid ahead and retract fields and legacy bodies', () => {
    expect(decodeServer(frame({ events: [{ ...event, id: 7 }], ahead: [{ ...event, id: 8 }], retract: [9] })).ok).toBe(true);
    expect(decodeServer(frame({ events: [event] })).ok).toBe(true);
  });

  it('rejects malformed ahead and retract values', () => {
    expect(decodeServer(frame({ events: [], ahead: {} })).ok).toBe(false);
    expect(decodeServer(frame({ events: [], retract: [-1] })).ok).toBe(false);
    expect(decodeServer(frame({ events: [], retract: [1.5] })).ok).toBe(false);
    expect(decodeServer(frame({ events: [], ahead: [{ ...event, id: 2 ** 53 }] })).ok).toBe(false);
    expect(decodeServer(frame({ events: [{ ...event, id: 2 ** 53 }] })).ok).toBe(false);
  });

  it('bounds committed plus ahead events and retract ids', () => {
    const batch = Array.from({ length: MAX_PLAYING_EVENTS }, () => event);
    expect(decodeServer(frame({ events: batch, ahead: [event] })).ok).toBe(false);
    expect(decodeServer(frame({ events: [], retract: Array.from({ length: MAX_PLAYING_EVENTS + 1 }, (_, i) => i) })).ok).toBe(false);
  });
});

describe('live-performance protocol envelopes', () => {
  it('accepts known transport output states and rejects unknown states', () => {
    const transport = {
      epoch: '1', sample_time: 0, cycle: [0, 1], bpm: 120, beats_per_cycle: 4, running: false,
      latency_seconds: null, latency_kind: 'unavailable', uncertainty_seconds: null,
    };
    const tempo = (output: string): string => JSON.stringify({ v: 1, seq: 1, kind: 'tempo', body: {
      bpm: 120, beats_per_cycle: 4, cycle: [0, 1], transport: { ...transport, output },
    } });
    expect(decodeServer(tempo('draining')).ok).toBe(true);
    expect(decodeServer(tempo('paused')).ok).toBe(false);
  });

  it('validates stop-all and momentary client frames', () => {
    expect(decodeClient(clientFrame('stop-all', {})).ok).toBe(true);
    expect(decodeClient(clientFrame('momentary', {
      file: 'song.vact', id: 1, form_gen: 2, edit_epoch: 3, target: null, ramp_ms: 1000,
    })).ok).toBe(true);
  });

  it('accepts the new stale reasons and rejects unknown reasons', () => {
    const stale = (reason: string): string => JSON.stringify({ v: 1, seq: 1, kind: 'stale-binding', body: {
      target: 1, reason,
    } });
    expect(decodeServer(stale('momentary-ineligible')).ok).toBe(true);
    expect(decodeServer(stale('unknown')).ok).toBe(false);
  });
});
