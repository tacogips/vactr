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
  if (raw.v !== PROTOCOL_VERSION) return fail('unsupported-version', `v = ${String(raw.v)}`);
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

/** Decodes one server frame; never throws. */
export function decodeServer(text: string): Decoded<ServerEnvelope> {
  return decodeWith<ServerEnvelope>(text, SERVER_KINDS, SERVER_FIELDS);
}

/** Decodes one client frame (tests and the recording transport). */
export function decodeClient(text: string): Decoded<ClientEnvelope> {
  return decodeWith<ClientEnvelope>(text, CLIENT_KINDS, CLIENT_FIELDS);
}
