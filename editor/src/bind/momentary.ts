import type { MomentaryGesture, MomentaryProvider, MomentaryStart } from '../app/apis';
import type { EditorDecl, ParamMeta } from '../protocol/types';
import { DRAG_PX, DRAG_SLOP_PX } from './drag';
import { SiteTable, type SiteEntry } from './sites';
import { fromUnit, isIntegerLiteral, paramMeta, round6, toUnit } from './write';

export const DRAG_SMOOTH_MS = 30, MIN_SNAP_MS = 0, MAX_GESTURES = 16, CONTEXT_WINDOW_MS = 500, TWO_FINGER_WINDOW_MS = 250;
type Key = string;
interface Active { key: Key; bindingId: string; y0: number; moved: boolean; startBase: number; unitOffset?: number; valueOffset?: number; lastTarget: number; lastId: number; lastGen: number; lastBase: number }
interface DisplayRamp { bindingId: string; from: number; to: number; started: number; duration: number }
export interface MomentaryHost { table: SiteTable; editors(): readonly EditorDecl[] | undefined; send(siteId: number, formGen: number, target: number | null, rampMs: number): void; glideMs(): number; now(): number; notice(m: string): void }
const clamp01 = (v: number): number => Math.max(0, Math.min(1, v));

export class MomentaryController implements MomentaryProvider {
  private active = new Map<Key, Active>();
  private ramps = new Map<string, DisplayRamp>();
  private serial = 0;
  constructor(private readonly host: MomentaryHost) {}
  private siteAt(pos: number): SiteEntry | undefined {
    let best: { entry: SiteEntry; width: number } | undefined;
    for (const entry of this.host.table.all()) {
      const range = this.host.table.currentRange(entry);
      if (!range || pos < range.from || pos > range.to) continue;
      const width = range.to - range.from;
      if (!best || width < best.width) best = { entry, width };
    }
    return best?.entry;
  }
  hit(pos: number): boolean { return this.siteAt(pos)?.state === 'bound'; }
  begin(start: MomentaryStart): MomentaryGesture | null {
    if (this.active.size >= MAX_GESTURES) return null;
    const entry = this.siteAt(start.pos);
    if (!entry) return null;
    if (entry.state !== 'bound' || entry.site.tier !== 'direct') { this.host.notice('momentary tweak needs a live site (re-evaluate or use a direct literal)'); return null; }
    const key = `${start.kind}:${this.serial++}`;
    const active: Active = { key, bindingId: entry.bindingId, y0: start.clientY, moved: false, startBase: entry.site.value, lastTarget: entry.site.value,
      lastId: entry.site.id, lastGen: entry.site.form_gen, lastBase: entry.site.value };
    this.active.set(key, active);
    return { move: (y) => this.move(active, y), end: (snap) => this.end(active, snap) };
  }
  private entry(a: Active): SiteEntry | undefined { const e = this.host.table.get(a.bindingId); return e?.state === 'bound' && e.site.tier === 'direct' ? e : undefined; }
  private meta(e: SiteEntry): ParamMeta | undefined { return paramMeta(e.site, this.host.editors()); }
  private target(a: Active, e: SiteEntry): number {
    const base = e.site.value, meta = this.meta(e);
    const raw = meta ? fromUnit(clamp01(toUnit(base, meta) + (a.unitOffset ?? 0)), meta) : base + (a.valueOffset ?? 0);
    return isIntegerLiteral(e.literalText) ? Math.round(raw) + 0 : round6(raw);
  }
  private move(a: Active, y: number): void {
    const dy = a.y0 - y;
    if (!a.moved && Math.abs(dy) < DRAG_SLOP_PX) return;
    const e = this.entry(a); if (!e) return;
    if (!a.moved) {
      a.moved = true;
      if (this.meta(e)) a.unitOffset = dy / DRAG_PX;
      else a.valueOffset = dy * 0.01 * Math.max(Math.abs(a.startBase), 1);
    } else if (a.unitOffset !== undefined) a.unitOffset = dy / DRAG_PX;
    else a.valueOffset = dy * 0.01 * Math.max(Math.abs(a.startBase), 1);
    const target = this.target(a, e);
    a.lastTarget = target; a.lastBase = e.site.value; a.lastId = e.site.id; a.lastGen = e.site.form_gen;
    this.host.send(e.site.id, e.site.form_gen, target, DRAG_SMOOTH_MS);
    this.ramps.delete(a.bindingId);
  }
  private end(a: Active, snap: boolean): void {
    this.active.delete(a.key);
    if (!a.moved) return;
    const e = this.entry(a);
    const duration = snap ? MIN_SNAP_MS : this.host.glideMs();
    this.host.send(e ? e.site.id : a.lastId, e ? e.site.form_gen : a.lastGen, null, duration);
    if (e && duration > 0 && (this.ramps.has(a.bindingId) || this.ramps.size < MAX_GESTURES)) this.ramps.set(a.bindingId, { bindingId: a.bindingId, from: a.lastTarget, to: e.site.value, started: this.host.now(), duration });
  }
  refresh(): void {
    for (const a of this.active.values()) {
      const e = this.entry(a);
      if (!e) {
        // A moved gesture stays active so pointerup can release its last bound identity.
        if (!a.moved) this.active.delete(a.key);
        continue;
      }
      if (a.lastId !== e.site.id || a.lastGen !== e.site.form_gen || a.lastBase !== e.site.value) {
        if (a.moved) { const target = this.target(a, e); this.host.send(e.site.id, e.site.form_gen, target, DRAG_SMOOTH_MS); a.lastTarget = target; }
        a.lastId = e.site.id; a.lastGen = e.site.form_gen; a.lastBase = e.site.value;
      }
    }
    for (const [id, ramp] of this.ramps) { const e = this.host.table.get(id); if (!e || e.state !== 'bound') this.ramps.delete(id); else ramp.to = e.site.value; }
  }
  rows(frameMs: number) {
    const rows = [] as { from: number; to: number; kind: 'momentary'; label: string }[];
    const add = (id: string, value: number): void => {
      const e = this.host.table.get(id), range = e?.state === 'bound' ? this.host.table.currentRange(e) : null;
      if (!e || !range) return;
      const meta = this.meta(e); const shown = meta?.curve === 'stepped' ? Math.round(value) : Number(value.toPrecision(3));
      rows.push({ from: range.to, to: range.to, kind: 'momentary', label: ` ~ ${String(shown)}` });
    };
    for (const a of this.active.values()) { const e = this.entry(a); if (e) add(a.bindingId, a.moved ? this.target(a, e) : e.site.value); }
    for (const [id, ramp] of this.ramps) {
      const t = ramp.duration <= 0 ? 1 : Math.min(1, Math.max(0, (frameMs - ramp.started) / ramp.duration));
      if (t >= 1) this.ramps.delete(id); else add(id, ramp.from + (ramp.to - ramp.from) * t);
    }
    return rows;
  }
}
