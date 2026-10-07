// Inline diagnostics (design 15.1.5): three sources merged into one
// presentation ranges are owned by the canvas surface.
//
// - static: the `diagnostics` of the file's latest `eval-result`, at that
//   reply's `doc_revision`;
// - check (browser tier only): `WasmCore.check` of the current text, run
//   300 ms after the last edit, at the revision it checked;
// - runtime: `diag` entries (per slot, shown with slot and beat), at the
//   revision of the latest `eval-result` when they arrived; a `clear` for a
//   slot removes that slot's entries.
//
// Every span is mapped from its revision to the current text; an
// unmappable one is dropped.

import type { CodeAnnotation, CodeSurface } from '../app/apis';
import type { Tier } from '../app/deps';
import type { Client } from '../protocol/client';
import { defaultTimers, type Timers } from '../protocol/document';
import type { Diagnostic, Severity } from '../protocol/types';
import type { DocumentSync } from './sync';

export const CHECK_DEBOUNCE_MS = 300;
export const MAX_CHECK_DIAGNOSTICS = 1024;

function defaultAfterPresent(callback: () => void): () => void {
  let frame: number | null = null;
  let task: ReturnType<typeof setTimeout> | null = null;
  let cancelled = false;
  const run = (): void => {
    if (cancelled) return;
    task = setTimeout(() => { task = null; if (!cancelled) callback(); }, 0);
  };
  if (typeof globalThis.requestAnimationFrame === 'function') frame = globalThis.requestAnimationFrame(run);
  else task = setTimeout(() => { task = null; if (!cancelled) callback(); }, 0);
  return () => {
    cancelled = true;
    if (frame !== null && typeof globalThis.cancelAnimationFrame === 'function') globalThis.cancelAnimationFrame(frame);
    if (task !== null) clearTimeout(task);
  };
}

export type DiagSource = 'static' | 'check' | 'runtime';

/** The part of `WasmCore` the check uses. */
export interface Checker {
  check(text: string): Diagnostic[];
}

interface Batch {
  rev: number;
  diags: Diagnostic[];
}

export interface DiagnosticsOptions {
  client: Client;
  sync: DocumentSync;
  tier: Tier;
  core?: Checker;
  timers?: Timers;
  /** The current document text (for the check). */
  text: () => string;
  afterPresent?: (callback: () => void) => () => void;
  isComposing?: () => boolean;
  announce?: (message: string) => void;
}

export interface PresentedDiagnostic extends CodeAnnotation { severity: Severity; message: string; source: string }

function runtimeMessage(d: Diagnostic): string {
  const where: string[] = [];
  if (d.slot !== undefined) where.push(`slot ${d.slot}`);
  if (d.beat) where.push(`beat ${d.beat[0]}/${d.beat[1]}`);
  return where.length > 0 ? `${d.message} (${where.join(', ')})` : d.message;
}

export class DiagnosticsController {
  private readonly opts: DiagnosticsOptions;
  private readonly timers: Timers;
  private surface: CodeSurface | null = null;
  private staticBatch: Batch | null = null;
  private checkBatch: Batch | null = null;
  private checkedRevision: number | null = null;
  private readonly runtime = new Map<string, Batch[]>();
  private evalRev: number | null = null;
  private timer: unknown = null;
  private pendingPresent: (() => void) | null = null;
  private inputSeq = 0;
  private merged: PresentedDiagnostic[] = [];
  private announcedCount = -1;
  private readonly offs: (() => void)[] = [];
  readonly stats = { checks: 0, checkDeferrals: 0, checkDropped: 0 };

  constructor(opts: DiagnosticsOptions) {
    this.opts = opts;
    this.timers = opts.timers ?? defaultTimers;
    const file = opts.sync.file;
    this.offs.push(
      opts.client.on('eval-result', (env) => {
        if (env.kind !== 'eval-result' || env.body.file !== file) return;
        this.evalRev = env.body.doc_revision;
        opts.sync.pin('diag:static', this.evalRev);
        this.staticBatch = { rev: env.body.doc_revision, diags: env.body.diagnostics.filter((d) => d.file === file) };
        this.refresh();
      }),
      opts.client.on('diag', (env) => {
        if (env.kind !== 'diag') return;
        for (const c of env.body.clear) { this.runtime.delete(c.slot); opts.sync.unpin(`diag:runtime:${c.slot}`); }
        const rev = this.evalRev;
        if (rev !== null) {
          for (const d of env.body.add) {
            if (d.file !== file) continue;
            const slot = d.slot ?? '';
            opts.sync.pin(`diag:runtime:${slot}`, rev);
            this.runtime.set(slot, [...(this.runtime.get(slot) ?? []), { rev, diags: [d] }]);
          }
        }
        this.refresh();
      }),
      opts.sync.onChange(() => this.scheduleCheck()),
    );
  }

  get checksEnabled(): boolean {
    return this.opts.tier === 'browser' && this.opts.core !== undefined;
  }

  attach(surface: CodeSurface): void {
    this.surface = surface;
    this.refresh();
  }

  /** The merged diagnostics last pushed to the view. */
  current(): readonly PresentedDiagnostic[] {
    return this.merged;
  }

  diagnosticAt(pos: number): PresentedDiagnostic | null {
    return this.merged.find((diagnostic) => diagnostic.from <= pos && pos <= diagnostic.to) ?? null;
  }

  /** Runs the typing-time check now (browser tier only). */
  runCheck(): void {
    this.disarm();
    const core = this.opts.core;
    if (!this.checksEnabled || !core) return;
    const rev = this.opts.sync.revision;
    if (rev === this.checkedRevision) return;
    let diags: Diagnostic[];
    try {
      this.stats.checks += 1;
      diags = core.check(this.opts.text());
    } catch {
      return;
    }
    this.checkedRevision = rev;
    // `session_check` checks exactly the text it was given.
    this.checkBatch = { rev, diags: diags.slice(0, MAX_CHECK_DIAGNOSTICS) };
    this.stats.checkDropped += Math.max(0, diags.length - MAX_CHECK_DIAGNOSTICS);
    this.opts.sync.pin('diag:check', rev);
    this.refresh();
  }

  /** Recomputes the merged set and pushes it to the view. */
  refresh(): void {
    const out: PresentedDiagnostic[] = [];
    const seen = new Set<string>();
    const add = (b: Batch | null, source: DiagSource): void => {
      if (!b) return;
      for (const d of b.diags) {
        const r = this.opts.sync.mapWireSpan(d.span, b.rev);
        if (!r) continue;
        const message = source === 'runtime' ? runtimeMessage(d) : d.message;
        const key = `${r.from}:${r.to}:${d.severity}:${message}`;
        if (seen.has(key)) continue;
        seen.add(key);
        out.push({ from: r.from, to: r.to, kind: 'diagnostic', className: `vact-diag-${d.severity}`,
          label: message, severity: d.severity, message, source: `${source}:${d.code}` });
      }
    };
    add(this.staticBatch, 'static');
    add(this.checkBatch, 'check');
    for (const batches of this.runtime.values()) for (const b of batches) add(b, 'runtime');
    out.sort((a, b) => a.from - b.from || a.to - b.to);
    this.merged = out;
    this.surface?.annotate('diagnostics', out);
    if (this.announcedCount !== out.length) {
      this.announcedCount = out.length;
      this.opts.announce?.(out.length === 1 ? '1 diagnostic' : `${out.length} diagnostics`);
    }
  }

  dispose(): void {
    this.disarm();
    this.cancelPendingPresent();
    for (const off of this.offs) off();
    this.offs.length = 0;
    this.opts.sync.unpin('diag:static');
    this.opts.sync.unpin('diag:check');
    for (const slot of this.runtime.keys()) this.opts.sync.unpin(`diag:runtime:${slot}`);
    this.runtime.clear();
    this.surface?.annotate('diagnostics', []);
    this.surface = null;
  }

  private scheduleCheck(): void {
    if (!this.checksEnabled) return;
    this.disarm();
    this.cancelPendingPresent(true);
    this.timer = this.timers.set(() => {
      this.timer = null;
      const snapshot = { inputSeq: this.inputSeq, revision: this.opts.sync.revision };
      this.pendingPresent = (this.opts.afterPresent ?? defaultAfterPresent)(() => {
        this.pendingPresent = null;
        if (!this.checksEnabled) return;
        if (snapshot.inputSeq !== this.inputSeq || snapshot.revision !== this.opts.sync.revision || (this.opts.isComposing?.() ?? false)) {
          this.stats.checkDeferrals += 1;
          this.scheduleCheck();
          return;
        }
        this.runCheck();
      });
    }, CHECK_DEBOUNCE_MS);
  }

  noteInput(): void {
    if (!this.checksEnabled) return;
    this.inputSeq += 1;
    this.scheduleCheck();
  }

  private cancelPendingPresent(countDeferral = false): void {
    if (!this.pendingPresent) return;
    this.pendingPresent();
    this.pendingPresent = null;
    if (countDeferral) this.stats.checkDeferrals += 1;
  }

  private disarm(): void {
    if (this.timer !== null) {
      this.timers.clear(this.timer);
      this.timer = null;
    }
  }
}
