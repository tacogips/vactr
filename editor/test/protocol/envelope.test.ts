import { describe, expect, it } from 'vitest';
import { decodeServer, MAX_PLAYING_EVENTS } from '../../src/protocol/envelope';
import type { WirePlaying } from '../../src/protocol/types';

const event: WirePlaying = { slot: 'd1', beat: [0, 1], time: 1, dur: [1, 8] };
const frame = (body: Record<string, unknown>): string => JSON.stringify({ v: 1, seq: 1, kind: 'playing', body });

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
