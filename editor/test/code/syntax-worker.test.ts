// @vitest-environment node

import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';
import { ChangeSet, EditorState, Text } from '@codemirror/state';
import { Language, Parser, Query } from 'web-tree-sitter';
import { WorkerSyntaxSpans, type SyntaxWorkerPort } from '../../src/code/syntax';
import { createVactSyntax, styleSpans, type VactSyntax } from '../../src/code/syntax-core';
import { SyntaxWorkerCore, type SyntaxWorkerReply, type SyntaxWorkerRequest } from '../../src/code/syntax-worker-core';

interface NodeFs { readFileSync(path: string): Uint8Array<ArrayBuffer> | string }
const proc = (globalThis as unknown as { process: { cwd(): string } }).process;
let fs: NodeFs;
let syntax: VactSyntax;
let parser: Parser;
let query: Query;

async function readFs(): Promise<NodeFs> {
  const spec: string = 'node:fs'; return (await import(/* @vite-ignore */ spec)) as NodeFs;
}
function readText(path: string): string {
  const bytes = fs.readFileSync(path); return typeof bytes === 'string' ? bytes : new TextDecoder().decode(bytes);
}

class FakePort implements SyntaxWorkerPort {
  readonly replies: SyntaxWorkerReply[] = [];
  readonly requests: SyntaxWorkerRequest[] = [];
  readonly listeners = new Map<string, EventListener[]>();
  readonly deferred: (() => void)[] = [];
  readonly terminate = vi.fn();
  readonly core: SyntaxWorkerCore;
  constructor(loader: (base: string) => Promise<VactSyntax> = async () => syntax) {
    this.core = new SyntaxWorkerCore({ post: (reply) => this.replies.push(reply), load: loader, defer: (cb) => this.deferred.push(cb) });
  }
  postMessage(request: SyntaxWorkerRequest): void { this.requests.push(request); this.core.handle(request); }
  addEventListener(type: 'message' | 'error' | 'messageerror', listener: EventListener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  emit(reply: SyntaxWorkerReply): void {
    for (const listener of this.listeners.get('message') ?? []) listener(new MessageEvent('message', { data: reply }));
  }
  error(type: 'error' | 'messageerror' = 'error'): void {
    for (const listener of this.listeners.get(type) ?? []) listener(new Event(type));
  }
  drain(): void { while (this.deferred.length) this.deferred.shift()!(); }
  deliverAll(): void { for (const reply of this.replies.splice(0)) this.emit(reply); }
}

describe('syntax Worker protocol', () => {
  beforeAll(async () => {
    fs = await readFs();
    const root = proc.cwd(); const runtimePath = `${root}/node_modules/web-tree-sitter/web-tree-sitter.wasm`;
    await Parser.init({ locateFile: () => runtimePath });
    const grammar = fs.readFileSync(`${root}/../tree-sitter-vact/tree-sitter-vact.wasm`);
    if (typeof grammar === 'string') throw new Error('WASM must be read as bytes');
    const language = await Language.load(grammar); parser = new Parser(); parser.setLanguage(language);
    query = new Query(language, readText(`${root}/../tree-sitter-vact/queries/highlights.scm`));
    syntax = createVactSyntax(parser, query);
  });
  afterAll(() => { query?.delete(); parser?.delete(); });

  it('keeps worker captures equivalent to a fresh parse through seeded edits and multi-change transactions', async () => {
    const port = new FakePort(); const changed = vi.fn();
    const provider = new WorkerSyntaxSpans(port, 'http://example.test/', changed);
    const lines = Array.from({ length: 320 }, (_, i) => `let value${i} ${i} # line ${i}`).join('\n');
    let state = EditorState.create({ doc: lines });
    await Promise.resolve(); port.deliverAll();
    provider.spans(state, 0, state.doc.length, 16_384); port.drain(); port.deliverAll();
    const random = seeded(0x3035);
    for (let i = 0; i < 200; i += 1) {
      const specs: { from: number; to?: number; insert: string }[] = [];
      const count = i % 4 === 0 ? 2 + Math.floor(random() * 4) : 1;
      let cursor = 0;
      for (let j = 0; j < count; j += 1) {
        const at = cursor + Math.floor(random() * Math.max(1, state.doc.length - cursor));
        const from = Math.min(state.doc.length, at), to = Math.min(state.doc.length, from + (random() < .35 ? 1 : 0));
        const inserts = ['x', '日本語', '😀', '\nslot :drums', ''];
        specs.push({ from, to, insert: inserts[Math.floor(random() * inserts.length)]! }); cursor = to + 1;
      }
      const changes = ChangeSet.of(specs, state.doc.length);
      const next = state.update({ changes }).state;
      provider.noteChanges(changes, next); state = next;
      if (i % 5 === 4) {
        port.drain(); port.deliverAll();
        const currentSeq = port.requests.filter((request) => request.type === 'edit').at(-1);
        port.postMessage({ type: 'lines', seq: currentSeq?.type === 'edit' ? currentSeq.seq : 0, first: 0, last: state.doc.lines - 1 });
        port.deliverAll();
        const current = provider.spans(state, 0, state.doc.length, 16_384).spans.map(({ from, to, className }) => ({ from, to, className }));
        const fresh = syntax.parseDoc(state.doc, null);
        expect(current, `seeded edit group ending at ${i}`).toEqual(styleSpans(fresh, 0, state.doc.length).map((span) => ({ from: span.from, to: span.to, className: span.cls })));
        fresh.delete();
      }
    }
    port.drain(); port.deliverAll();
    const currentSeq = port.requests.filter((request) => request.type === 'edit').at(-1);
    port.postMessage({ type: 'lines', seq: currentSeq?.type === 'edit' ? currentSeq.seq : 0, first: 0, last: state.doc.lines - 1 });
    port.deliverAll();
    const actual = provider.spans(state, 0, state.doc.length, 16_384).spans.map(({ from, to, className }) => ({ from, to, className }));
    const fresh = syntax.parseDoc(state.doc, null);
    const expected = styleSpans(fresh, 0, state.doc.length).map((span) => ({ from: span.from, to: span.to, className: span.cls }));
    const actualKeys = new Set(actual.map((span) => `${span.from}:${span.to}:${span.className}`));
    const missing = expected.filter((span) => !actualKeys.has(`${span.from}:${span.to}:${span.className}`));
    const expectedKeys = new Set(expected.map((span) => `${span.from}:${span.to}:${span.className}`));
    const extra = actual.filter((span) => !expectedKeys.has(`${span.from}:${span.to}:${span.className}`));
    expect({ missing: missing.length, firstMissing: missing[0], extra: extra.length, firstExtra: extra[0] }).toEqual({ missing: 0, firstMissing: undefined, extra: 0, firstExtra: undefined });
    fresh.delete(); provider.dispose();
  });

  it('applies multi-change offsets against one final document and remaps coalesced pending lines', async () => {
    const port = new FakePort(); const core = port.core;
    core.handle({ type: 'init', base: 'http://example.test/' }); await Promise.resolve(); port.replies.length = 0;
    core.handle({ type: 'reset', seq: 1, text: 'a\nb\nc\nd', window: [0, 3] }); port.drain();
    const reply = port.replies.at(-1); expect(reply?.type).toBe('spans');
    const doc = Text.of(['a', 'b', 'c', 'd']);
    const change = ChangeSet.of([{ from: 2, to: 3, insert: 'X\nY' }, { from: 6, to: 7, insert: 'Z' }], doc.length);
    core.handle({ type: 'edit', seq: 2, changes: specs(change), window: [0, 4] }); port.drain();
    const updated = change.apply(doc); const full = syntax.parseDoc(updated, null);
    const current = port.replies.at(-1); expect(current?.type).toBe('spans');
    const expectedSpans = styleSpans(full, 0, updated.length); full.delete();
    expect(expectedSpans).toEqual([]);
    expect([...port.replies].filter((row) => row.type === 'spans').at(-1)?.seq).toBe(2);

    const captured: number[] = [];
    const recordingSyntax: VactSyntax = {
      parse: () => ({ captures: () => [], delete() {} }),
      parseDoc: () => ({ captures(from, to) { captured.push(from); return []; }, changedRanges: () => [], delete() {} }),
      edit() {},
    };
    const pending = new FakePort(async () => recordingSyntax); pending.core.handle({ type: 'init', base: 'x' }); await Promise.resolve(); pending.replies.length = 0;
    const longDoc = Array.from({ length: 10 }, (_, i) => `line${i}`).join('\n');
    pending.core.handle({ type: 'reset', seq: 1, text: longDoc, window: [0, 9] }); pending.drain(); pending.replies.length = 0; captured.length = 0;
    pending.core.handle({ type: 'edit', seq: 2, changes: [[30, 31, 'Z']], window: [0, 9] });
    pending.core.handle({ type: 'edit', seq: 3, changes: [[5, 5, '\n']], window: [0, 10] });
    pending.drain(); const spans = pending.replies.filter((row) => row.type === 'spans').at(-1);
    expect(spans, JSON.stringify(pending.replies)).toMatchObject({ type: 'spans', seq: 3 });
    expect(captured).toContain(31); expect(captured).not.toContain(30);
  });

  it('captures exactly the final-document touched lines in the fixed multi-change reproduction', async () => {
    const captured: number[] = [];
    const recordingSyntax: VactSyntax = {
      parse: () => ({ captures: () => [], delete() {} }),
      parseDoc: () => ({ captures(from, to) { captured.push(from); return []; }, changedRanges: () => [], delete() {} }),
      edit() {},
    };
    const port = new FakePort(async () => recordingSyntax);
    port.core.handle({ type: 'init', base: 'x' }); await Promise.resolve(); port.replies.length = 0;
    port.core.handle({ type: 'reset', seq: 1, text: 'a\nb\nc\nd', window: [0, 3] }); port.drain(); captured.length = 0;
    port.core.handle({ type: 'edit', seq: 2, changes: [[2, 3, 'X\nY'], [6, 7, 'Z']], window: [0, 4] }); port.drain();
    const reply = port.replies.filter((row) => row.type === 'spans').at(-1);
    expect(reply?.type).toBe('spans');
    if (reply?.type !== 'spans') throw new Error('worker spans reply missing');
    expect(Array.from(reply.lines).filter((_value, index) => index % 3 === 0)).toEqual([1, 2, 4]);
    expect(captured).toEqual([2, 4, 8]);
  });

  it('coalesces twenty edits into one current reply and drops non-current replies', async () => {
    const port = new FakePort(); const provider = new WorkerSyntaxSpans(port, 'x');
    await Promise.resolve(); port.deliverAll();
    let state = EditorState.create({ doc: 'let value 1' }); provider.spans(state, 0, state.doc.length, 100);
    port.drain(); port.deliverAll();
    const before = provider.stats.replies;
    for (let i = 0; i < 20; i += 1) {
      const changes = ChangeSet.of([{ from: state.doc.length, insert: 'x' }], state.doc.length);
      state = state.update({ changes }).state; provider.noteChanges(changes, state);
    }
    port.drain(); expect(port.replies.filter((row) => row.type === 'spans')).toHaveLength(1);
    port.deliverAll(); expect(provider.stats.replies).toBe(before + 1); expect(provider.stats.staleReplies).toBe(0);
    provider.dispose();
  });

  it('drops a delivered stale reply without changing the mapped cache', async () => {
    const port = new FakePort(); const provider = new WorkerSyntaxSpans(port, 'x');
    await Promise.resolve(); port.deliverAll();
    let state = EditorState.create({ doc: 'let value 1' }); provider.spans(state, 0, state.doc.length, 100);
    port.drain(); port.deliverAll();
    const staleEdit = ChangeSet.of([{ from: 0, insert: '#' }], state.doc.length);
    state = state.update({ changes: staleEdit }).state; provider.noteChanges(staleEdit, state);
    port.drain();
    const stale = port.replies.splice(0).find((reply) => reply.type === 'spans');
    expect(stale?.type).toBe('spans');
    const currentEdit = ChangeSet.of([{ from: 0, insert: 'x' }], state.doc.length);
    state = state.update({ changes: currentEdit }).state; provider.noteChanges(currentEdit, state);
    const beforeStale = provider.spans(state, 0, state.doc.length, 100).spans;
    port.emit(stale!);
    expect(provider.stats.staleReplies).toBe(1);
    expect(provider.spans(state, 0, state.doc.length, 100).spans).toEqual(beforeStale);
    port.drain(); port.deliverAll(); provider.dispose();
  });

  it('keeps mapped Worker spans visible while an incremental reply is pending', async () => {
    const port = new FakePort(); const provider = new WorkerSyntaxSpans(port, 'x');
    await Promise.resolve(); port.deliverAll();
    let state = EditorState.create({ doc: 'let value 1' }); provider.spans(state, 0, state.doc.length, 100);
    const reset = port.requests.find((request) => request.type === 'reset');
    port.emit({ type: 'spans', seq: reset?.type === 'reset' ? reset.seq : 0,
      lines: new Uint32Array([0, 0, 1]), spans: new Uint32Array([0, 3, 0]), truncated: false });
    const changes = ChangeSet.of([{ from: 0, insert: 'x' }], state.doc.length);
    state = state.update({ changes }).state; provider.noteChanges(changes, state);
    expect(provider.spans(state, 0, state.doc.length, 100).spans[0]?.className).toBe('vact-tok-comment');
    provider.dispose();
  });

  it('sends one edit for the keystroke path and resets only beyond batch bounds', async () => {
    const port = new FakePort(); const provider = new WorkerSyntaxSpans(port, 'x');
    await Promise.resolve(); port.deliverAll();
    const state = EditorState.create({ doc: 'a'.repeat(70_000) }); provider.spans(state, 0, 1, 10); port.drain(); port.deliverAll();
    const before = port.requests.length;
    const beforeResets = provider.stats.resets;
    const edit = ChangeSet.of([{ from: 0, insert: 'b' }], state.doc.length);
    const next = state.update({ changes: edit }).state;
    const toString = vi.spyOn(Text.prototype, 'toString'); provider.noteChanges(edit, next);
    expect(port.requests.length).toBe(before + 1); expect(port.requests.at(-1)?.type).toBe('edit');
    expect(provider.stats.resets).toBe(beforeResets);
    expect(toString.mock.contexts.filter((doc) => (doc as Text).length >= 65_536)).toHaveLength(0);
    toString.mockRestore();
    const huge = ChangeSet.of([{ from: 0, insert: 'x'.repeat(65_537) }], next.doc.length);
    const hugeState = next.update({ changes: huge }).state;
    const resetStringify = vi.spyOn(Text.prototype, 'toString');
    provider.noteChanges(huge, hugeState);
    expect(resetStringify.mock.contexts.filter((doc) => (doc as Text).length >= 65_536)).toHaveLength(1);
    resetStringify.mockRestore();
    expect(port.requests.at(-1)?.type).toBe('reset'); expect(provider.stats.resets).toBe(beforeResets + 1);
    const many = ChangeSet.of(Array.from({ length: 1_025 }, (_, index) => ({ from: index * 2, insert: 'y' })), hugeState.doc.length);
    provider.noteChanges(many, hugeState.update({ changes: many }).state);
    expect(port.requests.at(-1)?.type).toBe('reset'); expect(provider.stats.resets).toBe(beforeResets + 2); provider.dispose();
  });

  it('reports truncation, worker errors, ready timeout and loader failure', async () => {
    const many = 'x'.repeat(20_000);
    const synthetic: VactSyntax = {
      parse: () => ({ captures: () => [], delete() {} }),
      parseDoc: (doc) => ({ captures(from, to) {
        const end = Math.min(to, from + 20_000), rows = [];
        for (let pos = from; pos < end; pos += 1) rows.push({ name: 'keyword', from: pos, to: pos + 1 });
        return rows;
      }, delete() {} }),
      edit() {},
    };
    const syntaxMany = synthetic;
    const port = new FakePort(async () => syntaxMany); const provider = new WorkerSyntaxSpans(port, 'x');
    await Promise.resolve(); port.deliverAll(); const state = EditorState.create({ doc: many }); provider.spans(state, 0, state.doc.length, 16_384);
    port.drain(); port.deliverAll(); expect(provider.spans(state, 0, state.doc.length, 16_384).truncated).toBe(true); provider.dispose();
    const failing = new FakePort(); const onFail = vi.fn(); const failed = new WorkerSyntaxSpans(failing, 'x', () => {}, onFail);
    failing.error(); failing.error('messageerror'); expect(failing.terminate).toHaveBeenCalledTimes(1);
    expect(onFail).toHaveBeenCalledTimes(1); expect(failed.stats.workerFailures).toBe(1);
    vi.useFakeTimers();
    try { const timeoutPort = new FakePort(); const timed = new WorkerSyntaxSpans(timeoutPort, 'x', () => {}, onFail); vi.advanceTimersByTime(10_000); expect(timed.stats.workerFailures).toBe(1); timed.dispose(); }
    finally { vi.useRealTimers(); }
    const out: SyntaxWorkerReply[] = []; const rejectCore = new SyntaxWorkerCore({ post: (r) => out.push(r), load: async () => { throw new Error('load failed'); } });
    expect(() => rejectCore.handle({ type: 'init', base: 'x' })).not.toThrow(); await Promise.resolve(); expect(out.at(-1)).toEqual({ type: 'failed', reason: 'load failed' });
  });
});

function specs(change: ChangeSet): [number, number, string][] {
  const rows: [number, number, string][] = [];
  change.iterChanges((from, to, _fromB, _toB, inserted) => rows.push([from, to, inserted.toString()])); return rows;
}
function seeded(seed: number): () => number {
  let value = seed; return () => { value = (value * 1664525 + 1013904223) >>> 0; return value / 0x1_0000_0000; };
}
