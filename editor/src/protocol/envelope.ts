// Envelope encode/decode (command.md "Envelope"): `{v, seq, kind, body, re?}`
// with `v = 1`. Decoding never throws: a bad frame becomes a typed error
// value the client reports and drops, and the connection stays usable.

import {
  CLIENT_KINDS,
  PROTOCOL_VERSION,
  SERVER_KINDS,
  type ClientEnvelope,
  type ClientKind,
  type ClientMsg,
  type ServerEnvelope,
  type ServerKind,
} from './types';

export type DecodeErrorCode = 'bad-json' | 'unsupported-version' | 'unknown-kind' | 'bad-shape';

export interface DecodeError {
  code: DecodeErrorCode;
  message: string;
}

export type Decoded<T> = { ok: true; env: T } | { ok: false; error: DecodeError };

type FieldType = 'array' | 'number' | 'string' | 'object' | 'boolean';

// The required top-level body fields per server kind: enough that a
// consumer (the store in particular) never walks a missing array.
const SERVER_FIELDS: Record<ServerKind, Record<string, FieldType>> = {
  'clock-probe': { page_send: 'number', engine_receive: 'number', engine_send: 'number', epoch: 'string' },
  'eval-result': {
    file: 'string',
    doc_revision: 'number',
    forms: 'array',
    diagnostics: 'array',
    sites: 'array',
    directives: 'object',
  },
  'stale-binding': { reason: 'string' },
  'directive-edit': {
    file: 'string',
    doc_revision: 'number',
    span: 'object',
    expected: 'string',
    text: 'string',
  },
  manifest: { sounds: 'array', synths: 'array', controls: 'array' },
  'protocol-error': { code: 'string', message: 'string' },
  bindings: { pass: 'number', changed: 'array', sites: 'array', states: 'array' },
  diag: { add: 'array', clear: 'array' },
  playing: { events: 'array' },
  levels: { levels: 'array' },
  tempo: { bpm: 'number', beats_per_cycle: 'number', cycle: 'array' },
};

const CLIENT_FIELDS: Record<ClientKind, Record<string, FieldType>> = {
  'clock-probe': { page_send: 'number' },
  eval: { file: 'string', code: 'string', doc_revision: 'number', edit_epoch: 'number' },
  hush: {},
  stop: { slot: 'string' },
  'set-var': { file: 'string', name: 'string', defining_form_gen: 'number', edit_epoch: 'number' },
  'set-tweak': {
    file: 'string',
    id: 'number',
    form_gen: 'number',
    value: 'number',
    edit_epoch: 'number',
  },
  'doc-changed': {
    file: 'string',
    doc_revision: 'number',
    base_revision: 'number',
    changes: 'array',
    dirty: 'array',
    edit_epoch: 'number',
  },
  learn: { file: 'string', cc: 'number', edit_epoch: 'number' },
  subscribe: { telemetry: 'boolean', levels: 'boolean', diagnostics: 'boolean' },
  'manifest?': {},
};

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null && !Array.isArray(x);
}

function hasType(x: unknown, t: FieldType): boolean {
  switch (t) {
    case 'array':
      return Array.isArray(x);
    case 'object':
      return isObject(x);
    default:
      return typeof x === t;
  }
}

function fail<T>(code: DecodeErrorCode, message: string): Decoded<T> {
  return { ok: false, error: { code, message } };
}

function decodeWith<T>(
  text: string,
  kinds: readonly string[],
  fields: Record<string, Record<string, FieldType>>,
): Decoded<T> {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (e) {
    return fail('bad-json', String(e));
  }
  if (!isObject(raw)) return fail('bad-shape', 'the envelope is not an object');
  if (raw.v !== PROTOCOL_VERSION) return fail('unsupported-version', `v = ${JSON.stringify(raw.v)}`);
  if (typeof raw.seq !== 'number' || !Number.isInteger(raw.seq)) {
    return fail('bad-shape', 'seq is not an integer');
  }
  if (raw.re !== undefined && (typeof raw.re !== 'number' || !Number.isInteger(raw.re))) {
    return fail('bad-shape', 're is not an integer');
  }
  const kind = raw.kind;
  if (typeof kind !== 'string' || !kinds.includes(kind)) {
    return fail('unknown-kind', `kind ${JSON.stringify(kind)}`);
  }
  const body = raw.body;
  if (!isObject(body)) return fail('bad-shape', `${kind}: the body is not an object`);
  const need = fields[kind] ?? {};
  for (const [name, t] of Object.entries(need)) {
    if (!hasType(body[name], t)) return fail('bad-shape', `${kind}: body.${name} is not ${t}`);
  }
  return { ok: true, env: raw as T };
}

/** One client envelope as JSON text. */
export function encodeClient(seq: number, msg: ClientMsg): string {
  return JSON.stringify({ v: PROTOCOL_VERSION, seq, kind: msg.kind, body: msg.body });
}

/** One server envelope as JSON text (the recording transport's replies). */
export function encodeServer(env: ServerEnvelope): string {
  const out: Record<string, unknown> = {
    v: env.v,
    seq: env.seq,
    kind: env.kind,
    body: env.body,
  };
  if (env.re !== undefined) out.re = env.re;
  return JSON.stringify(out);
}

export const MAX_TELEMETRY_BYTES = 1024 * 1024;
export const MAX_PLAYING_EVENTS = 4096;

const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const nonnegative = (v: unknown): v is number => finite(v) && v >= 0;
const positive = (v: unknown): v is number => finite(v) && v > 0;
const integer = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
const epoch = (v: unknown): boolean => typeof v === 'string' && v.length > 0;
function ratio(v: unknown, duration = false): boolean {
  return Array.isArray(v) && v.length === 2 && Number.isSafeInteger(v[0]) &&
    Number.isSafeInteger(v[1]) && v[1] > 0 && (!duration || v[0] >= 0);
}
function latency(v: Record<string, unknown>): boolean {
  return (v.latency_seconds === null || nonnegative(v.latency_seconds)) &&
    (v.uncertainty_seconds === null || nonnegative(v.uncertainty_seconds)) &&
    typeof v.latency_kind === 'string' && ['measured', 'estimate', 'unavailable'].includes(v.latency_kind) &&
    (v.latency_kind === 'unavailable' ? v.latency_seconds === null : v.latency_seconds !== null);
}
function transport(v: unknown): boolean {
  return isObject(v) && epoch(v.epoch) && nonnegative(v.sample_time) && ratio(v.cycle) &&
    positive(v.bpm) && positive(v.beats_per_cycle) && typeof v.running === 'boolean' && latency(v);
}
function validPlaying(v: unknown): boolean {
  if (!isObject(v) || typeof v.slot !== 'string' || !nonnegative(v.time) ||
    !ratio(v.beat) || !ratio(v.dur, true)) return false;
  if (v.epoch !== undefined && !epoch(v.epoch)) return false;
  if (v.end_time !== undefined && (!nonnegative(v.end_time) || v.end_time < v.time)) return false;
  if (v.src !== undefined) {
    const s = v.src;
    if (!isObject(s) || typeof s.file !== 'string' || !integer(s.doc_revision) ||
      !integer(s.form_gen) || !isObject(s.span) || !integer(s.span.start) ||
      !integer(s.span.end) || s.span.end < s.span.start) return false;
  }
  return true;
}

/** Decodes one server frame; never throws. */
export function decodeServer(text: string): Decoded<ServerEnvelope> {
  const decoded = decodeWith<ServerEnvelope>(text, SERVER_KINDS, SERVER_FIELDS);
  if (!decoded.ok) return decoded;
  const env = decoded.env;
  if (['playing', 'levels', 'tempo'].includes(env.kind) &&
    new TextEncoder().encode(text).byteLength > MAX_TELEMETRY_BYTES) {
    return fail('bad-shape', 'telemetry exceeds 1 MiB');
  }
  switch (env.kind) {
    case 'playing':
      if (env.body.events.length > MAX_PLAYING_EVENTS || !env.body.events.every(validPlaying))
        return fail('bad-shape', 'invalid or oversized playing batch');
      break;
    case 'levels':
      if (!env.body.levels.every((v) => isObject(v) && typeof v.source === 'string' &&
        nonnegative(v.rms) && (v.bands === undefined ||
        (Array.isArray(v.bands) && v.bands.every(nonnegative)))) ||
        (env.body.analyzers !== undefined && (!Array.isArray(env.body.analyzers) ||
        !env.body.analyzers.every((v) => isObject(v) && typeof v.bus === 'string' &&
          typeof v.kind === 'string' && integer(v.id) && Array.isArray(v.cells) && v.cells.every(finite)))))
        return fail('bad-shape', 'invalid levels');
      break;
    case 'tempo':
      if (!positive(env.body.bpm) || !positive(env.body.beats_per_cycle) || !ratio(env.body.cycle) ||
        (env.body.transport !== undefined && !transport(env.body.transport)))
        return fail('bad-shape', 'invalid tempo timing');
      break;
    case 'clock-probe': {
      const b = env.body;
      if (!nonnegative(b.page_send) || !nonnegative(b.engine_receive) || !nonnegative(b.engine_send) ||
        b.engine_send < b.engine_receive || !epoch(b.epoch) || !latency(b as unknown as Record<string, unknown>) ||
        (b.correlation !== undefined && (!isObject(b.correlation) ||
          !nonnegative(b.correlation.engine_time) || !nonnegative(b.correlation.output_time))))
        return fail('bad-shape', 'invalid clock probe');
      break;
    }
  }
  return decoded;
}

/** Decodes one client frame (tests and the recording transport). */
export function decodeClient(text: string): Decoded<ClientEnvelope> {
  const decoded = decodeWith<ClientEnvelope>(text, CLIENT_KINDS, CLIENT_FIELDS);
  if (decoded.ok && decoded.env.kind === 'clock-probe' && !nonnegative(decoded.env.body.page_send))
    return fail('bad-shape', 'invalid clock probe page_send');
  return decoded;
}
