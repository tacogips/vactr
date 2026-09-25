// Opening a parameter editor (design 15.1.7 "Opening", 13.5).
//
// A CALL GROUP is the set of sites sharing `site.call` `{name, head,
// ordinal}` within one top-level form. The editor kind comes from
// `manifest.editors[name].kind`; a name with no entry (or a kind this
// editor does not know) gets `scalar`. Each declared parameter is bound
// to the site whose `call.param` names it, else whose `call.arg` is its
// index; a parameter with no site gets a null binding (a disabled handle,
// never a text insertion).

import type { EditorDecl, EditorKind, Span, WireForm, WireSite } from '../protocol/types';

export interface CallGroup {
  /** Stable across re-evaluations: `<form>:<name>:<ordinal>`. */
  id: string;
  name: string;
  head: Span;
  ordinal: number;
  /** Index into the file's `eval-result.forms`, or -1 when none contains the call. */
  form: number;
  /** Sites in `(arg, span.start)` order. */
  sites: WireSite[];
}

export const EDITOR_KINDS: readonly EditorKind[] = [
  'eq-curve',
  'filter-response',
  'dynamics-transfer',
  'envelope-shape',
  'delay-taps',
  'reverb-room',
  'sampler-wave',
  'wavetable-frames',
  'granular-region',
  'lfo-shape',
  'stereo-field',
  'xy-pad',
  'euclid-ring',
  'probability-dial',
  'length-handle',
  'scalar',
];

/** The sampler controls (13.5): their groups also offer the waveform editor. */
export const SAMPLE_CONTROLS: readonly string[] = ['begin', 'end', 'loop', 'n', 'slice', 'chop', 'striate', 'speed'];

const inside = (outer: Span, inner: Span): boolean => outer.start <= inner.start && inner.end <= outer.end;

/** The index of the innermost form containing `span`, or -1. */
export function formIndexOf(span: Span, forms: readonly WireForm[]): number {
  let best = -1;
  forms.forEach((f, i) => {
    if (!inside(f.span, span)) return;
    const cur = forms[best];
    if (!cur || f.span.end - f.span.start < cur.span.end - cur.span.start) best = i;
  });
  return best;
}

/** Groups the sites that carry `call` by (form, name, head, ordinal), in head order. */
export function callGroups(sites: readonly WireSite[], forms: readonly WireForm[] = []): CallGroup[] {
  const groups = new Map<string, CallGroup>();
  for (const s of sites) {
    const c = s.call;
    if (!c) continue;
    const form = formIndexOf(c.head, forms);
    const key = `${form}:${c.name}:${c.head.start}:${c.ordinal}`;
    let g = groups.get(key);
    if (!g) {
      g = { id: `${form}:${c.name}:${c.ordinal}`, name: c.name, head: c.head, ordinal: c.ordinal, form, sites: [] };
      groups.set(key, g);
    }
    g.sites.push(s);
  }
  const out = [...groups.values()];
  for (const g of out) g.sites.sort((a, b) => (a.call?.arg ?? 0) - (b.call?.arg ?? 0) || a.span.start - b.span.start);
  return out.sort((a, b) => a.head.start - b.head.start);
}

/** The editor declaration and kind for a group. */
export function editorFor(
  group: Pick<CallGroup, 'name'>,
  editors: readonly EditorDecl[] | undefined,
): { kind: EditorKind; decl?: EditorDecl } {
  const decl = editors?.find((d) => d.name === group.name);
  if (!decl) return { kind: 'scalar' };
  return { kind: EDITOR_KINDS.includes(decl.kind) ? decl.kind : 'scalar', decl };
}

/**
 * Binds each declared parameter to a site of the group: `call.param`
 * first, else `call.arg` as an index into `decl.params`. Without a
 * declaration every site is its own parameter (`param`, else `arg<n>`).
 */
export function bindHandles(group: Pick<CallGroup, 'sites'>, decl?: EditorDecl): Map<string, number | null> {
  const out = new Map<string, number | null>();
  if (!decl) {
    for (const s of group.sites) {
      let name = s.call?.param ?? `arg${s.call?.arg ?? 0}`;
      if (out.has(name)) name = `${name}.${s.id}`;
      out.set(name, s.id);
    }
    return out;
  }
  decl.params.forEach((p, i) => {
    const byParam = group.sites.find((s) => s.call?.param === p.name);
    const byArg = group.sites.find((s) => s.call?.param === undefined && s.call?.arg === i);
    out.set(p.name, (byParam ?? byArg)?.id ?? null);
  });
  return out;
}

/** The other call groups of the same top-level form (the chain). */
export function siblings(group: CallGroup, all: readonly CallGroup[]): CallGroup[] {
  if (group.form < 0) return [];
  return all.filter((g) => g !== group && g.form === group.form);
}

/** True when the group can open the sampler waveform editor. */
export function offersSampler(group: Pick<CallGroup, 'name'>, kind: EditorKind): boolean {
  return kind === 'sampler-wave' || SAMPLE_CONTROLS.includes(group.name);
}
