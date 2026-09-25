// Session Protocol v1 wire types (design-docs/specs/command.md "Session
// Protocol (v1)", "Browser transport (raw wasm ABI, TASK-010)").
//
// Every span and change offset is a UTF-8 BYTE offset in the document
// revision named alongside it; `utf8.ts` converts to and from the UTF-16
// offsets of CodeMirror. Fields marked TASK-010 are optional additions
// (design 15.1.2 G2-G5): `v` stays 1 and an old session omits them.
// This file is the single owner of the wire shapes; no later plan edits it.

export const PROTOCOL_VERSION = 1;

// ---------------------------------------------------------------- common

/** `{start, end}` UTF-8 byte offsets. */
export interface Span {
  start: number;
  end: number;
}

export interface SrcRef {
  file: string;
  span: Span;
  doc_revision: number;
  form_gen: number;
}

export type Severity = 'error' | 'warning' | 'hint';

/** A beat as `[num, den]`. */
export type Ratio = [number, number];

export interface Diagnostic {
  code: string;
  severity: Severity;
  message: string;
  span: Span;
  file: string;
  slot?: string;
  beat?: Ratio;
}

export type SiteTier = 'direct' | 'reeval' | 'manual';
export type SiteOrigin = 'pattern-literal' | 'binding' | 'inst-default';

/** The enclosing call of a site's literal (TASK-010, G3). */
export interface SiteCall {
  name: string;
  head: Span;
  /** 1-based among same-named calls in the top-level form. */
  ordinal: number;
  /** 0-based argument index. */
  arg: number;
  param?: string;
}

export interface WireSite {
  id: number;
  span: Span;
  tier: SiteTier;
  origin: SiteOrigin;
  value: number;
  form_gen: number;
  /** The BindingKey spelling `label.site.n.param` or `label.param`. */
  key?: string;
  /** TASK-010 (G3). */
  call?: SiteCall;
}

export type EditorKind =
  | 'eq-curve'
  | 'filter-response'
  | 'dynamics-transfer'
  | 'envelope-shape'
  | 'delay-taps'
  | 'reverb-room'
  | 'sampler-wave'
  | 'wavetable-frames'
  | 'granular-region'
  | 'lfo-shape'
  | 'stereo-field'
  | 'xy-pad'
  | 'euclid-ring'
  | 'probability-dial'
  | 'length-handle'
  | 'scalar';

export type ParamCurve = 'linear' | 'log' | 'stepped';
export type ParamUnit = 'none' | 'db' | 's' | 'ms' | 'hz' | 'st';

export interface ParamMeta {
  name: string;
  /** Control-table row; pattern functions carry none. */
  ctl?: number;
  range: [number, number];
  curve: ParamCurve;
  unit: ParamUnit;
  group: number;
}

/** One builtin's parameter-editor declaration (TASK-010, G2). */
export interface EditorDecl {
  name: string;
  kind: EditorKind;
  multiband?: boolean;
  params: ParamMeta[];
}

/** `{from, to, insert_len}` in base-revision bytes. */
export interface ByteChange {
  from: number;
  to: number;
  insert_len: number;
}

// ------------------------------------------------------ client to session

export interface EvalBody {
  file: string;
  /** The full document text. */
  code: string;
  span?: Span;
  doc_revision: number;
  edit_epoch: number;
}

export type EmptyBody = Record<string, never>;

export interface StopBody {
  slot: string;
}

export interface SetVarBody {
  file: string;
  name: string;
  value: number | boolean;
  defining_form_gen: number;
  edit_epoch: number;
}

export interface SetTweakBody {
  file: string;
  id: number;
  form_gen: number;
  value: number;
  edit_epoch: number;
}

export interface DocChangedBody {
  file: string;
  doc_revision: number;
  base_revision: number;
  changes: ByteChange[];
  /** New-revision spans. */
  dirty: Span[];
  edit_epoch: number;
}

export interface LearnBody {
  file: string;
  /** A BindingKey spelling or a tweak id. */
  binding: string | number;
  cc: number;
  ch?: number;
  edit_epoch: number;
}

export interface SubscribeBody {
  telemetry: boolean;
  levels: boolean;
  diagnostics: boolean;
}

export type ClientMsg =
  | { kind: 'eval'; body: EvalBody }
  | { kind: 'hush'; body: EmptyBody }
  | { kind: 'stop'; body: StopBody }
  | { kind: 'set-var'; body: SetVarBody }
  | { kind: 'set-tweak'; body: SetTweakBody }
  | { kind: 'doc-changed'; body: DocChangedBody }
  | { kind: 'learn'; body: LearnBody }
  | { kind: 'subscribe'; body: SubscribeBody }
  | { kind: 'manifest?'; body: EmptyBody };

export type ClientKind = ClientMsg['kind'];

export const CLIENT_KINDS: readonly ClientKind[] = [
  'eval',
  'hush',
  'stop',
  'set-var',
  'set-tweak',
  'doc-changed',
  'learn',
  'subscribe',
  'manifest?',
];

// ------------------------------------------------------ session to client

export interface WireForm {
  span: Span;
  value?: string;
  failure?: Diagnostic;
  form_gen: number;
}

export interface WireFileLevel {
  midi_ch?: number;
}

export interface WireDirective {
  span: Span;
  /** `positional` or `addressed`. */
  kind: string;
  target?: Span;
  trailing: boolean;
}

export interface WireLabel {
  name: string;
  spans: Span[];
  ambiguous: boolean;
}

export interface WireBinding {
  key?: string;
  span: Span;
  param: string;
  cc?: number;
  ch?: number;
  directive: Span;
}

export interface WireDirectives {
  file_level: WireFileLevel;
  entries: WireDirective[];
  labels?: WireLabel[];
  bindings?: WireBinding[];
}

export interface EvalResultBody {
  file: string;
  doc_revision: number;
  forms: WireForm[];
  diagnostics: Diagnostic[];
  /** Every site of `file`. */
  sites: WireSite[];
  directives: WireDirectives;
}

export type StaleReason =
  | 'stale-form-gen'
  | 'edit-invalidated'
  | 'unreconciled-edit'
  | 'superseded-definition';

export interface StaleBindingBody {
  /** A tweak id or a name. */
  target: number | string;
  reason: StaleReason;
  current_form_gen?: number;
}

export interface DirectiveEditBody {
  file: string;
  doc_revision: number;
  span: Span;
  expected: string;
  text: string;
}

export interface ManifestBody {
  sounds: string[];
  synths: string[];
  controls: string[];
  /** TASK-010 (G2): every builtin's EditorDecl plus the pattern functions. */
  editors?: EditorDecl[];
}

export type ProtocolErrorCode = 'bad-json' | 'unsupported-version' | 'unknown-kind' | 'bad-body';

export interface ProtocolErrorBody {
  code: ProtocolErrorCode;
  message: string;
}

export interface WireChanged {
  name: string;
  value: string;
  form_gen: number;
}

export type FormStateKind = 'ok' | 'failed' | 'blocked';

export interface WireFormState {
  name: string;
  state: FormStateKind;
  value: string;
  blocked_on?: string;
  diagnostic?: Diagnostic;
}

/** Exactly one per completed reactive pass. */
export interface BindingsBody {
  pass: number;
  changed: WireChanged[];
  /** Sites of the recomputed forms. */
  sites: WireSite[];
  states: WireFormState[];
}

export interface DiagBody {
  add: Diagnostic[];
  clear: { slot: string }[];
}

export interface WirePlaying {
  slot: string;
  beat: Ratio;
  time: number;
  dur: Ratio;
  src?: SrcRef;
}

export interface PlayingBody {
  events: WirePlaying[];
}

export interface WireLevel {
  source: string;
  rms: number;
  /** TASK-010 (G4): the 8 host FFT bands. */
  bands?: number[];
}

/** One analyzer unit of the installed bus graph (TASK-010, G4). */
export interface WireAnalyzer {
  bus: string;
  kind: string;
  id: number;
  cells: number[];
}

export interface LevelsBody {
  levels: WireLevel[];
  analyzers?: WireAnalyzer[];
}

export interface TempoClock {
  source: 'internal' | 'midi';
  locked?: boolean;
}

export interface TempoBody {
  bpm: number;
  beats_per_cycle: number;
  cycle: Ratio;
  /** TASK-010 (G4). */
  clock?: TempoClock;
}

export type ServerMsg =
  | { kind: 'eval-result'; body: EvalResultBody }
  | { kind: 'stale-binding'; body: StaleBindingBody }
  | { kind: 'directive-edit'; body: DirectiveEditBody }
  | { kind: 'manifest'; body: ManifestBody }
  | { kind: 'protocol-error'; body: ProtocolErrorBody }
  | { kind: 'bindings'; body: BindingsBody }
  | { kind: 'diag'; body: DiagBody }
  | { kind: 'playing'; body: PlayingBody }
  | { kind: 'levels'; body: LevelsBody }
  | { kind: 'tempo'; body: TempoBody };

export type ServerKind = ServerMsg['kind'];

export const SERVER_KINDS: readonly ServerKind[] = [
  'eval-result',
  'stale-binding',
  'directive-edit',
  'manifest',
  'protocol-error',
  'bindings',
  'diag',
  'playing',
  'levels',
  'tempo',
];

/** The body type of a server message kind. */
export type ServerBody<K extends ServerKind> = Extract<ServerMsg, { kind: K }>['body'];

// -------------------------------------------------------------- envelopes

export interface EnvelopeHead {
  v: number;
  seq: number;
  /** On replies only: the client `seq` being answered. */
  re?: number;
}

export type ClientEnvelope = ClientMsg & EnvelopeHead;
export type ServerEnvelope = ServerMsg & EnvelopeHead;

// ------------------------------------------- browser-local wasm records

/** Outbox record tags (command.md "Browser transport"). */
export const TAG_CONSOLE = 0x70;
export const TAG_SESSION = 0x71;
export const TAG_RENDER = 0x72;
export const TAG_PKG = 0x73;

/** The `session_check` reply, carried on `0x71` (not a protocol message). */
export interface SessionCheckRecord {
  kind: 'check';
  diagnostics: Diagnostic[];
}

export type OutputIndex = 0 | 1 | 2 | 3;

/** A `0x72` render record (G5). */
export type RenderRecord =
  | {
      op: 'program';
      out: OutputIndex;
      source: string;
      uniform_names: string[];
      assets: { id: number; text: string }[];
    }
  | { op: 'uniforms'; out: OutputIndex; values: number[] };

/** A `pkg_resolve` request (G6). */
export type PkgResolveRequest =
  | { proxy: string; requirements: Record<string, string> }
  | { proxy: string; lock: string };

export interface PkgResolved {
  path: string;
  version: string;
  sha256: string;
}

/** A `0x73` package driver reply (G6). */
export type PkgReply =
  | { status: 'need'; url: string }
  | { status: 'done'; lock: string; resolved: PkgResolved[] }
  | { status: 'error'; code: string; message: string };
