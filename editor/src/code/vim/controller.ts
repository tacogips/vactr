import { isolateHistory, redoDepth, undoDepth } from '@codemirror/commands';
import { Transaction, type ChangeSpec, type EditorSelection, type EditorState, type TransactionSpec } from '@codemirror/state';
import type { ExCommand } from './ex';
import { parseEx } from './ex';
import { changeWordEnd, commandInputTokens, isChanging, lineTargetRange, preserveChangeWordSeparator, repeatTokens, selectionForVisual, visualJoin, visualRange, visualReplaceChars, withoutCounts } from './execute';
import { isPassThrough, normalizeKey } from './keys';
import { MarkTable } from './marks';
import { motionRange, applyOperator, openLine, putRegister, replaceChars, toggleCaseChars } from './operators';
import { CommandParser, type ParsedCommand, type Target } from './parser';
import { MacroRecorder, RedoRecorder, textToTokens, tokensToText } from './record';
import { RegisterStore } from './registers';
import { compilePattern, findMatch, substitute, wordPattern, wordUnderCursor } from './search';
import { UndoLedger } from './undo';
import type { FindState, MotionResult, RecordToken, VimHost, VimInputHook, VimKey, VimKeyInput, VimMode, VimSnapshot } from './types';
import { REPLAY_DEPTH_LIMIT, REPLAY_TOKEN_LIMIT } from './types';
import { selectTextObject } from './textobjects';
import { clampNormal, firstNonBlank, graphemeColumn, nextGrapheme, prevGrapheme } from './words';
import { InsertSession, isTextEvent, modeAllowsText } from './session';

const labels: Record<VimMode, string> = { normal: 'NORMAL', insert: 'INSERT', replace: 'REPLACE', visual: 'VISUAL', 'visual-line': 'VISUAL LINE', 'operator-pending': 'NORMAL' };
const isolated = (event: string): Pick<TransactionSpec, 'annotations'> => ({ annotations: [isolateHistory.of('full'), Transaction.userEvent.of(event)] });
const documentWord = 'doc' + 'ument';

export class VimController implements VimInputHook {
  private mode: VimMode = 'normal';
  private parser = new CommandParser();
  private registers = new RegisterStore();
  private marks = new MarkTable();
  private ledger = new UndoLedger();
  private redo = new RedoRecorder();
  private macro = new MacroRecorder();
  private session = new InsertSession();
  private listeners = new Set<(snapshot: VimSnapshot) => void>();
  private unsubscribe: () => void;
  private disposed = false;
  private selfDispatch = false;
  private hadHistory = false;
  private desiredColumn: number | null = null;
  private lastFind: FindState | null = null;
  private lastSearch: { regex: RegExp; direction: 1 | -1 } | null = null;
  private lastPlayed: string | null = null;
  private visual: { anchor: number; head: number } | null = null;
  private lastVisual: { fromLine: number; toLine: number } | null = null;
  private replayDepth = 0;
  private replaying = false;
  private repeatCount: number | null = null;
  private lastSequence: RecordToken[] = [];
  private snapshotValue: VimSnapshot;

  constructor(private readonly host: VimHost) {
    const state = host.state(), cursor = clampNormal(state.doc, state.selection.main.head);
    this.snapshotValue = { mode: this.mode, label: labels[this.mode], pending: '', recording: null, search: null, message: null, messageKind: null, prompt: null, cursorShape: 'block', displayCursor: cursor, displayCursorEnd: nextGrapheme(state.doc, cursor) };
    this.unsubscribe = host.subscribe((update) => this.onUpdate(update));
  }

  get snapshot(): VimSnapshot { return { ...this.snapshotValue, prompt: this.snapshotValue.prompt ? { ...this.snapshotValue.prompt } : null }; }

  subscribe(cb: (snapshot: VimSnapshot) => void): () => void {
    if (this.disposed) return () => undefined;
    this.listeners.add(cb);
    cb(this.snapshot);
    return () => this.listeners.delete(cb);
  }

  handleKey(input: VimKeyInput): boolean {
    if (this.disposed || this.host.compositionActive()) return false;
    if (isPassThrough(input, this.host.platform, this.host.matchesDefaultKeymap)) return false;
    const key = normalizeKey(input);
    if (key === null) return false;
    if (this.mode === 'insert' || this.mode === 'replace') return this.handleSessionKey(key, input);
    if (key === '<Esc>' && input.ctrlKey && this.host.closePopupWithEscape()) return true;
    if (this.macro.register && this.parser.pending() === '' && key === 'q') {
      const stopped = this.macro.stop();
      if (stopped) { const unnamed = this.registers.read('"'); this.registers.write(stopped.reg, { text: '', linewise: false, tokens: stopped.tokens }, 'yank'); if (unnamed) this.registers.write('"', unnamed, 'delete'); }
      this.refresh();
      return true;
    }
    if (this.macro.register && !this.replaying) this.macro.push({ kind: 'key', key });
    this.lastSequence = textToTokens(this.parser.pending() + key);
    const result = this.parser.feed(key, this.mode);
    if (result.kind === 'pending') {
      this.mode = this.parser.operatorPending() ? 'operator-pending' : this.visual ? (this.mode === 'visual-line' ? 'visual-line' : 'visual') : 'normal';
      this.refresh();
      return true;
    }
    if (result.kind === 'invalid') {
      this.mode = this.visual ? (this.mode === 'visual-line' ? 'visual-line' : 'visual') : 'normal';
      if (result.message) this.message(result.message, 'error');
      else this.refresh();
      return true;
    }
    this.execute(result.command);
    return true;
  }

  allowsTextInput(): boolean { return modeAllowsText(this.mode); }

  overwriteRange(text: string): { from: number; to: number } | null {
    if (this.mode !== 'replace' || !text) return null;
    const doc = this.host.state().doc;
    const cursor = this.host.state().selection.main.head;
    const line = doc.lineAt(cursor);
    let to = cursor;
    for (const _ of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) to = nextGrapheme(doc, to);
    to = Math.min(to, line.to);
    this.session.pushOverwrite(doc.sliceString(cursor, to));
    return { from: cursor, to };
  }

  noteCompositionDiscarded(): void { this.message(`IME input ignored in ${labels[this.mode]} mode`, 'info'); }

  submitPrompt(text: string): void {
    const prompt = this.snapshotValue.prompt;
    if (!prompt) return;
    if (prompt.kind === ':') this.executeEx(text);
    else {
      const source = text || (this.lastSearch ? this.lastSearch.regex.source : '');
      if (!source) this.message('E35 No previous regular expression', 'error');
      else {
        const compiled = compilePattern(source);
        const flags = compiled.regex.flags.includes('m') ? compiled.regex.flags : `${compiled.regex.flags}m`;
        this.lastSearch = { regex: new RegExp(compiled.regex.source, flags), direction: prompt.kind === '/' ? 1 : -1 };
        this.snapshotValue.search = `${prompt.kind}${source}`;
        if (compiled.note) this.message(compiled.note, 'info');
        else {
          const result = this.searchJump(this.lastSearch.direction, this.lastSearch.regex, this.cursor());
          if (result) { this.marks.setContext(this.cursor()); this.dispatch({ selection: { anchor: result.pos }, ...isolated('select.vim') }); this.host.scrollCaret(); }
        }
      }
    }
    this.snapshotValue.prompt = null;
    this.mode = this.visual ? this.mode : 'normal';
    this.refresh();
  }

  cancelPrompt(): void { if (this.snapshotValue.prompt) { this.snapshotValue.prompt = null; this.mode = this.visual ? (this.snapshotValue.mode === 'visual-line' ? 'visual-line' : 'visual') : 'normal'; this.refresh(); } }

  tapMode(): void {
    if (this.mode === 'insert' || this.mode === 'replace') this.exitSession(false);
    else this.execute({ register: null, count: null, operator: null, target: null, action: 'insert', arg: null });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.unsubscribe();
    this.listeners.clear();
    this.parser.reset();
    this.session.clear();
    this.redo.abort();
  }

  private handleSessionKey(key: VimKey, input: VimKeyInput): boolean {
    if (key === '<Esc>') {
      if (input.ctrlKey && this.host.closePopupWithEscape()) return true;
      this.pushExitToken();
      this.exitSession(true);
      return true;
    }
    if (this.mode === 'replace' && key === '<BS>') {
      const original = this.session.popOverwrite();
      if (original !== null) {
        const doc = this.host.state().doc, pos = this.cursor();
        const from = Math.max(this.host.state().doc.lineAt(pos).from, prevGrapheme(doc, pos));
        this.dispatch({ changes: { from, to: pos, insert: original }, selection: { anchor: from + original.length }, ...isolated('vim.replace-restore') });
      } else this.moveByKey('<Left>');
      return true;
    }
    if (['<Left>', '<Right>', '<Up>', '<Down>', '<Home>', '<End>'].includes(key)) {
      if (!this.replaying) {
        const moveKinds: Record<string, import('./types').MotionKind> = { '<Left>': 'h', '<Right>': 'l', '<Up>': 'k', '<Down>': 'j', '<Home>': '0', '<End>': '$' };
        const token: RecordToken = { kind: 'move', motion: moveKinds[key] };
        this.macro.push(token); this.redo.end();
      }
      return false;
    }
    return false;
  }

  private execute(command: ParsedCommand): void {
    const state = this.host.state();
    if (command.register && /^[1-9]$/.test(command.register)) { this.message('E354 Invalid register name', 'error'); this.finishMode(); return; }
    const wasVisual = this.mode === 'visual' || this.mode === 'visual-line';
    const tokens = this.lastSequence.length ? this.lastSequence.map((token) => ({ ...token })) : commandInputTokens(command, this.parser.pending());
    this.lastSequence = [];
    const changing = isChanging(command, this.mode);
    if (changing && !this.replaying && !wasVisual) { this.redo.begin(withoutCounts(tokens)); this.repeatCount = command.count; }
    if (command.operator || command.target) this.executeTarget(command);
    else this.executeAction(command);
    if (!this.session.mode && this.redo.active && !this.replaying) this.redo.end();
    this.refresh();
  }

  private executeTarget(command: ParsedCommand): void {
    const state = this.host.state(), doc = state.doc, entryDepth = undoDepth(state);
    const cursor = this.cursor();
    const wasVisual = this.mode === 'visual' || this.mode === 'visual-line';
    let result: MotionResult | null = null;
    let range = this.visual ? visualRange(state, this.visual.anchor, this.visual.head, this.mode) : null;
    const target = command.target;
    if (target?.kind === 'motion') {
      result = this.motion(target.motion, command.count);
      if (!result) { if (command.operator) this.finishMode(); return; }
      if (result.scrollLines) this.host.scrollLines(result.scrollLines);
      if (result.jump) this.marks.setContext(cursor);
      if (command.operator) {
        range = motionRange(doc, cursor, result, command.operator);
        if (target.motion.kind === 'l' && result.pos === cursor && cursor < doc.lineAt(cursor).to) range = { from: cursor, to: nextGrapheme(doc, cursor), linewise: false };
      }
      else if (this.visual) {
        this.visual.head = clampNormal(doc, result.pos);
        const visualSelection = selectionForVisual(state, visualRange(state, this.visual.anchor, this.visual.head, this.mode));
        this.dispatch({ selection: visualSelection, ...isolated('select.vim') }); this.host.scrollCaret();
        this.desiredColumn = result.desiredColumn; this.lastFind = result.lastFind ?? this.lastFind; return;
      } else {
        this.dispatch({ selection: { anchor: result.pos }, ...isolated('select.vim') }); this.host.scrollCaret();
        this.desiredColumn = result.desiredColumn; this.lastFind = result.lastFind ?? this.lastFind; return;
      }
      this.desiredColumn = result.desiredColumn; this.lastFind = result.lastFind ?? this.lastFind;
    } else if (target?.kind === 'textobject') {
      range = selectTextObject(doc, cursor, target.spec, command.count ?? 1, this.visual);
      if (!range) { this.finishMode(); return; }
      if (this.visual) {
        this.visual = selectionForVisual(state, range);
        this.mode = this.mode === 'visual-line' ? 'visual-line' : 'visual';
        this.dispatch({ selection: this.visual, ...isolated('select.vim') });
        return;
      }
    } else if (target?.kind === 'line') {
      range = lineTargetRange(state, cursor, command.count ?? 1, wasVisual ? this.visual : null);
    } else if (target?.kind === 'search' || target?.kind === 'mark') {
      const found = target.kind === 'search' ? this.resolveSearch(target.which, cursor) : this.resolveMark(target.name);
      if (!found) { this.finishMode(); return; }
      result = found;
      if (result.jump) this.marks.setContext(cursor);
      if (command.operator) range = motionRange(doc, cursor, result, command.operator);
      else {
        if (this.visual) this.visual.head = result.pos;
        else this.mode = 'normal';
        const selection = this.visual ? selectionForVisual(state, visualRange(state, this.visual.anchor, this.visual.head, this.mode)) : { anchor: result.pos };
        this.dispatch({ selection, ...isolated('select.vim') }); this.host.scrollCaret(); return;
      }
    }
    if (!command.operator) { this.finishMode(); return; }
    if (!range) { this.finishMode(); return; }
    if (command.operator === 'c' && (command.count === null || command.count === 1) && target?.kind === 'motion' && target.motion.kind === 'w') {
      const end = changeWordEnd(doc, cursor);
      if (end !== null) range = { from: cursor, to: end, linewise: false };
    } else if (command.operator === 'c' && (command.count === null || command.count === 1) && target?.kind === 'motion' && target.motion.kind === 'W') {
      const word = wordUnderCursor(doc, cursor);
      if (word && word.text && !/^\s/u.test(word.text)) {
        const end = this.motion({ kind: 'E' }, command.count); if (end) range = motionRange(doc, cursor, end, 'c');
      }
    } else if (command.operator === 'c' && target?.kind === 'motion' && ['w', 'W'].includes(target.motion.kind)) range = preserveChangeWordSeparator(doc, range, command.count ?? 1);
    const outcome = applyOperator(state, command.operator, range);
    if (!outcome.changes) {
      if (outcome.yanked) this.writeRegister(command.register, outcome.yanked, command.operator === 'y' ? 'yank' : 'delete');
      if (wasVisual) this.leaveVisual(range.from); else this.finishMode(); return;
    }
    if (range.from === range.to && !range.linewise) { this.finishMode(); return; }
    if (outcome.yanked) this.writeRegister(command.register, outcome.yanked, command.operator === 'y' ? 'yank' : 'delete');
    if (wasVisual) this.leaveVisual(outcome.cursor); this.dispatch({ changes: outcome.changes, selection: { anchor: outcome.cursor }, ...isolated(`vim.${command.operator === 'd' ? 'delete' : command.operator === 'c' ? 'change' : command.operator === 'y' ? 'yank' : command.operator === '>' ? 'indent' : command.operator === '<' ? 'outdent' : 'case'}`) });
    if (outcome.enterInsert) this.startSession('insert', 1, entryDepth, !wasVisual); else this.finishMode();
  }

  private executeAction(command: ParsedCommand): void {
    const action = command.action, count = command.count ?? 1;
    const state = this.host.state(), doc = state.doc, cursor = this.cursor(), line = doc.lineAt(cursor);
    switch (action) {
      case 'insert': case 'append': case 'insert-line-start': case 'append-line-end': {
        let pos = cursor;
        if (action === 'append' && line.length) pos = nextGrapheme(doc, cursor);
        if (action === 'insert-line-start') pos = firstNonBlank(doc, line.number);
        if (action === 'append-line-end') pos = line.to;
        if (pos !== cursor) this.dispatch({ selection: { anchor: pos }, ...isolated('select.vim') });
        this.startSession('insert', count); break;
      }
      case 'open-below': case 'open-above': {
        const opened = openLine(state, cursor, action === 'open-above');
        const depth = undoDepth(state);
        this.dispatch({ changes: opened.changes, selection: { anchor: opened.cursor }, ...isolated('vim.open-line') });
        this.startSession('insert', count, depth); break;
      }
      case 'replace-mode': this.startSession('replace', count); break;
      case 'visual': case 'visual-line': {
        if (this.visual) { this.leaveVisual(this.visual.head); break; }
        this.visual = { anchor: cursor, head: cursor };
        this.mode = action === 'visual-line' ? 'visual-line' : 'visual';
        this.dispatch({ selection: this.visual, ...isolated('select.vim') }); break;
      }
      case 'visual-swap': if (this.visual) { [this.visual.anchor, this.visual.head] = [this.visual.head, this.visual.anchor]; this.dispatch({ selection: this.visual, ...isolated('select.vim') }); } break;
      case 'put-after': case 'put-before': this.put(command.register ?? '"', action === 'put-before', count); break;
      case 'replace-char': case 'toggle-case': {
        const visualRangeValue = this.visual && action === 'replace-char' ? visualRange(state, this.visual.anchor, this.visual.head, this.mode) : null;
        const outcome = visualRangeValue ? visualReplaceChars(state, visualRangeValue, command.arg ?? '') : action === 'replace-char' ? replaceChars(state, cursor, command.arg ?? '', count) : toggleCaseChars(state, cursor, count);
        if (!outcome) this.redo.abort(); else { if (visualRangeValue) this.leaveVisual(visualRangeValue.from); this.dispatch({ changes: outcome.changes, selection: { anchor: outcome.cursor }, ...isolated(action === 'replace-char' ? 'vim.replace' : 'vim.case') }); }
        break;
      }
      case 'join': {
        const { outcome, range } = visualJoin(state, cursor, count, this.visual);
        if (range) this.leaveVisual(range.from);
        if (outcome) this.dispatch({ changes: outcome.changes, selection: { anchor: outcome.cursor }, ...isolated('vim.join') }); else this.redo.abort();
        break;
      }
      case 'undo': for (let i = 0; i < count; i++) this.ledger.undo(this.host); break;
      case 'redo': for (let i = 0; i < count; i++) this.ledger.redo(this.host); break;
      case 'repeat': this.repeat(command.count); break;
      case 'record': this.macro.start(command.arg ?? 'a'); this.snapshotValue.recording = command.arg; break;
      case 'play': this.play(command.arg ?? '@', count); break;
      case 'set-mark': this.marks.set(command.arg ?? 'a', cursor); break;
      case 'prompt': {
        if (this.visual && command.arg === ':') {
          const range = visualRange(state, this.visual.anchor, this.visual.head, this.mode);
          this.lastVisual = { fromLine: doc.lineAt(range.from).number, toLine: doc.lineAt(range.to).number };
          this.leaveVisual(this.visual.head);
          this.snapshotValue.prompt = { kind: ':', initial: "'<,'>" };
        } else this.snapshotValue.prompt = { kind: (command.arg ?? ':') as ':' | '/' | '?', initial: '' };
        break;
      }
      case 'escape': if (this.visual) this.leaveVisual(this.visual.head); else { this.parser.reset(); this.message(''); } break;
      default: this.finishMode();
    }
  }

  private motion(spec: { kind: import('./types').MotionKind; char?: string }, count: number | null): MotionResult | null {
    const state = this.host.state();
    return importMotion(state.doc, this.cursor(), spec, { count: count ?? 1, explicitCount: count !== null, desiredColumn: this.desiredColumn, viewport: this.host.viewport(), lastFind: this.lastFind });
  }

  private resolveSearch(which: 'n' | 'N' | '*' | '#', cursor: number): MotionResult | null {
    let search = this.lastSearch;
    if (which === '*' || which === '#') {
      const word = wordUnderCursor(this.host.state().doc, cursor);
      if (!word) { this.message('E486 Pattern not found: ', 'error'); return null; }
      const direction = which === '*' ? 1 : -1;
      search = { regex: wordPattern(word.text), direction };
      this.lastSearch = search; this.snapshotValue.search = `${direction > 0 ? '/' : '?'}${word.text}`;
    }
    if (!search) { this.message('E35 No previous regular expression', 'error'); return null; }
    const direction = which === 'N' ? (search.direction === 1 ? -1 : 1) : which === 'n' ? search.direction : search.direction;
    return this.searchJump(direction, search.regex, cursor);
  }

  private searchJump(direction: 1 | -1, regex: RegExp, cursor: number): MotionResult | null {
    const found = findMatch(this.host.state().doc, cursor, regex, direction);
    if (!found) { this.message(`E486 Pattern not found: ${regex.source}`, 'error'); return null; }
    if (found.wrapped) this.message(direction === 1 ? 'search hit BOTTOM, continuing at TOP' : 'search hit TOP, continuing at BOTTOM', 'info');
    return { pos: found.from, linewise: false, inclusive: false, desiredColumn: null, jump: true };
  }

  private resolveMark(name: string): MotionResult | null {
    const pos = name === "'" || name === '`' ? this.marks.context() : this.marks.get(name);
    if (pos === null) { this.message('E20 Mark not set', 'error'); return null; }
    const target = name === "'" ? firstNonBlank(this.host.state().doc, this.host.state().doc.lineAt(pos).number) : pos;
    return { pos: target, linewise: name === "'", inclusive: false, desiredColumn: null, jump: true };
  }

  private writeRegister(name: string | null, value: import('./types').Register, kind: 'yank' | 'delete'): void {
    this.registers.write(name, value, kind);
    if (name === '+' || name === '*') void this.host.clipboard.write(value.text).catch(() => this.message('clipboard unavailable; kept in the internal register', 'error'));
  }

  private put(name: string, before: boolean, count: number): void {
    if ((name === '+' || name === '*') && !this.replaying) {
      const doc = this.host.state().doc, selection = this.host.state().selection, mode = this.mode;
      void this.host.clipboard.read().then((text) => {
        if (this.host.state().doc !== doc || this.host.state().selection !== selection || this.mode !== mode) { this.message(`clipboard paste dropped: ${documentWord} changed`, 'error'); return; }
        this.registers.setMirror(text, false);
        this.putRegisterNow(name, before, count, { text, linewise: false }, true);
      }).catch(() => {
        const mirror = this.registers.read(name);
        if (mirror) this.putRegisterNow(name, before, count, mirror);
        this.message('clipboard unavailable; kept in the internal register', 'error');
      });
      return;
    }
    const value = this.registers.read(name);
    if (value) this.putRegisterNow(name, before, count, value.tokens ? { text: tokensToText(value.tokens), linewise: value.linewise } : value);
  }

  private putRegisterNow(name: string, before: boolean, count: number, value: import('./types').Register, redoAsEdit = false): void {
    const state = this.host.state(), cursor = this.cursor();
    const outcome = putRegister(state, cursor, value, before, count);
    if (redoAsEdit && !this.replaying) {
      const transaction = state.update({ changes: outcome.changes });
      const ranges: { fromA: number; toA: number; fromB: number; toB: number }[] = [];
      transaction.changes.iterChangedRanges((fromA, toA, fromB, toB) => ranges.push({ fromA, toA, fromB, toB }));
      if (ranges.length === 1) {
        const range = ranges[0];
        const token: RecordToken = { kind: 'edit', deleteBefore: cursor - range.fromA, deleteAfter: range.toA - cursor, insert: transaction.state.doc.sliceString(range.fromB, range.toB) };
        this.redo.abort(); this.redo.begin([token]);
      }
    }
    this.dispatch({ changes: outcome.changes, selection: { anchor: outcome.cursor }, ...isolated('vim.put') });
    if (redoAsEdit && !this.replaying) this.redo.end();
    this.refresh();
  }

  private executeEx(text: string): void {
    const state = this.host.state(), currentLine = state.doc.lineAt(this.cursor()).number;
    const parsed = parseEx(text, { currentLine, lastLine: state.doc.lines, visual: this.lastVisual });
    if ('error' in parsed) { this.message(parsed.error, 'error'); return; }
    this.applyEx(parsed);
  }

  private applyEx(command: ExCommand): void {
    if (command.kind === 'empty') return;
    if (command.kind === 'goto') {
      const old = this.cursor(); this.marks.setContext(old);
      const pos = firstNonBlank(this.host.state().doc, command.line);
      this.dispatch({ selection: { anchor: pos }, ...isolated('select.vim') }); this.host.scrollCaret(); return;
    }
    if (command.kind === 'noh') { this.snapshotValue.search = null; return; }
    if (command.kind === 'noop-write') { this.message(`vactr syncs the ${documentWord} live; use :eval, :evalall or :hush`, 'info'); return; }
    if (command.kind === 'action') { this.host.actions[command.action](); return; }
    if (command.kind === 'substitute') {
      const pattern = command.pattern || this.lastSearch?.regex.source;
      if (!pattern) { this.message('E35 No previous regular expression', 'error'); return; }
      const compiled = compilePattern(pattern), regex = new RegExp(compiled.regex.source, `${compiled.regex.flags}${command.ignoreCase && !compiled.regex.flags.includes('i') ? 'i' : ''}`);
      const outcome = substitute(this.host.state().doc, command.fromLine, command.toLine, regex, command.replacement, command.global);
      if (!outcome.count) this.message(`E486 Pattern not found: ${pattern}`, 'error');
      else {
        this.dispatch({ changes: outcome.changes, ...isolated('vim.substitute') });
        this.message(`${outcome.count} substitutions on ${outcome.lines} lines`, 'info');
      }
    }
  }

  private startSession(mode: 'insert' | 'replace', count: number, entryDepth = undoDepth(this.host.state()), recordsDot = true): void {
    const cursor = this.host.state().selection.main.head;
    this.dispatch({ selection: this.host.state().selection, ...isolated('vim.insert') });
    this.session.start(mode, count, entryDepth, cursor, recordsDot);
    this.mode = mode; this.parser.reset(); this.announceMode();
  }

  private pushExitToken(): void {
    if (this.replaying) return;
    const esc: RecordToken = { kind: 'key', key: '<Esc>' };
    this.macro.push(esc); this.redo.push(esc);
  }

  private exitSession(keyDriven: boolean): void {
    const active = this.session.mode;
    if (!active) return;
    const tokens = this.session.tokens();
    const times = Math.max(1, this.session.count);
    if (!this.replaying && !keyDriven) this.pushExitToken();
    if (times > 1) for (let n = 1; n < times; n++) this.replayEdits(tokens);
    this.dispatch({ selection: this.host.state().selection, ...isolated('vim.insert') });
    if (!this.replaying) this.ledger.record({ entryDepth: this.session.entryDepth, exitDepth: undoDepth(this.host.state()), exitDoc: this.host.state().doc });
    if (!this.replaying) this.redo.end();
    const doc = this.host.state().doc, pos = this.host.state().selection.main.head, line = doc.lineAt(pos);
    this.dispatch({ selection: { anchor: Math.max(line.from, prevGrapheme(doc, pos)) }, ...isolated('select.vim') });
    this.session.clear(); this.mode = 'normal'; this.parser.reset(); this.announceMode();
  }

  private replayEdits(tokens: readonly RecordToken[]): void {
    for (const token of tokens) if (token.kind === 'edit') {
      const pos = this.cursor(), doc = this.host.state().doc;
      const from = Math.max(0, pos - token.deleteBefore), to = Math.min(doc.length, pos + token.deleteAfter);
      this.dispatch({ changes: { from, to, insert: token.insert }, selection: { anchor: from + token.insert.length }, ...isolated('vim.repeat') });
    }
  }

  private repeat(count: number | null): void {
    const recorded = this.redo.last();
    if (!recorded) return;
    const tokens = repeatTokens(recorded, count ?? this.repeatCount);
    this.replaying = true; this.replayDepth++;
    if (this.replayDepth > REPLAY_DEPTH_LIMIT || tokens.length > REPLAY_TOKEN_LIMIT) { this.message('E169 Command too recursive', 'error'); this.replaying = false; this.replayDepth = 0; return; }
    const start = this.host.state(), entryDepth = undoDepth(start);
    try {
      this.replayTokens(tokens);
    } finally {
      if (this.session.mode) this.exitSession(false);
      this.parser.reset(); this.mode = 'normal';
      this.dispatch({ selection: this.host.state().selection, ...isolated('vim.repeat') });
      this.ledger.record({ entryDepth, exitDepth: undoDepth(this.host.state()), exitDoc: this.host.state().doc });
      this.replaying = false; this.replayDepth = 0; this.refresh();
    }
  }

  private replayTokens(tokens: readonly RecordToken[]): void {
    for (const token of tokens) {
      if (token.kind === 'edit') {
        const pos = this.cursor(), doc = this.host.state().doc;
        this.dispatch({ changes: { from: Math.max(0, pos - token.deleteBefore), to: Math.min(doc.length, pos + token.deleteAfter), insert: token.insert }, selection: { anchor: Math.max(0, pos - token.deleteBefore) + token.insert.length }, ...isolated('vim.repeat') });
        if (this.session.mode) this.session.recordToken(token);
      } else if (token.kind === 'move') this.moveByMotion(token.motion);
      else if (this.mode === 'insert' || this.mode === 'replace') {
        if (token.key === '<Esc>') this.exitSession(true);
        else if (token.key === '<CR>' || token.key === '<BS>' || token.key === '<Del>' || [...token.key].length === 1) {
          const pos = this.cursor(), doc = this.host.state().doc;
          const change: ChangeSpec = token.key === '<CR>' ? { from: pos, insert: '\n' } : token.key === '<BS>' ? { from: prevGrapheme(doc, pos), to: pos, insert: '' } : token.key === '<Del>' ? { from: pos, to: nextGrapheme(doc, pos), insert: '' } : { from: pos, insert: token.key };
          this.dispatch({ changes: change, selection: { anchor: change.from + (token.key === '<CR>' ? 1 : token.key.startsWith('<') ? 0 : token.key.length) }, ...isolated('vim.repeat') });
        }
      } else {
        if (token.key === '<Esc>') { this.parser.reset(); this.mode = 'normal'; continue; }
        const normalized: VimKey = token.key;
        this.lastSequence = textToTokens(this.parser.pending() + normalized);
        const result = this.parser.feed(normalized, this.mode);
        if (result.kind === 'command') this.execute(result.command);
        else if (result.kind === 'invalid') { this.parser.reset(); break; }
      }
    }
  }

  private play(name: string, count: number): void {
    const actual = name === '@' ? this.lastPlayed : name;
    if (!actual) { this.message('E748 No previously used register', 'error'); return; }
    const register = this.registers.read(actual);
    if (!register) { this.message(`E475 Invalid argument: ${actual}`, 'error'); return; }
    const tokens = register.tokens ?? textToTokens(register.text);
    this.lastPlayed = actual;
    const wasReplaying = this.replaying;
    this.replaying = true; this.replayDepth++;
    if (this.replayDepth > REPLAY_DEPTH_LIMIT || tokens.length * count > REPLAY_TOKEN_LIMIT) { this.message('E169 Command too recursive', 'error'); this.replayDepth--; this.replaying = wasReplaying; return; }
    try { for (let n = 0; n < count; n++) this.replayTokens(tokens); }
    finally { this.replayDepth--; this.replaying = wasReplaying; this.refresh(); }
  }

  private moveByMotion(kind: import('./types').MotionKind): void {
    const result = this.motion({ kind }, null);
    if (result) this.dispatch({ selection: { anchor: result.pos }, ...isolated('select.vim') });
  }

  private moveByKey(key: string): void {
    const mapping: Record<string, import('./types').MotionKind> = { '<Left>': 'h', '<Right>': 'l', '<Up>': 'k', '<Down>': 'j' };
    const kind = mapping[key]; if (kind) this.moveByMotion(kind);
  }

  private leaveVisual(head: number): void {
    if (this.visual) {
      const range = visualRange(this.host.state(), this.visual.anchor, this.visual.head, this.mode);
      this.lastVisual = { fromLine: this.host.state().doc.lineAt(range.from).number, toLine: this.host.state().doc.lineAt(range.to).number };
    }
    this.visual = null; this.mode = 'normal'; this.parser.reset();
    this.dispatch({ selection: { anchor: head }, ...isolated('select.vim') });
  }

  private finishMode(): void { if (this.visual) this.mode = this.mode === 'visual-line' ? 'visual-line' : 'visual'; else this.mode = 'normal'; this.parser.reset(); }

  private cursor(): number { const head = this.host.state().selection.main.head; return this.mode === 'insert' || this.mode === 'replace' ? head : clampNormal(this.host.state().doc, head); }

  private dispatch(spec: TransactionSpec): void {
    this.selfDispatch = true;
    try { this.host.dispatch(spec); } finally { this.selfDispatch = false; }
  }

  private onUpdate(update: import('./types').VimUpdate): void {
    if (update.docChanged) {
      this.marks.map(update.changes);
      this.ledger.noteChange(update.userEvent === 'undo' || update.userEvent === 'redo');
      if (this.session.mode && isTextEvent(update) && !this.selfDispatch) {
        const token = this.session.track(update);
        if (token && !this.replaying) {
          if (this.session.recordsDot) { if (!this.redo.active) this.redo.begin([{ kind: 'key', key: 'i' }]); this.redo.push(token); }
          this.macro.push(token);
        }
      }
    }
    const reduced = this.host.historyReduced();
    if (reduced && !this.hadHistory) this.ledger.clear();
    this.hadHistory = reduced;
    if (!this.selfDispatch && update.selectionSet) this.externalSelection(update.state.selection);
    if (this.session.mode) this.session.rebaseHead(update.state.selection.main.head);
    this.refresh();
  }

  private externalSelection(selection: EditorSelection): void {
    if (this.mode === 'normal' && !selection.main.empty) {
      this.visual = { anchor: selection.main.from, head: prevGrapheme(this.host.state().doc, selection.main.to) };
      this.mode = 'visual';
    } else if (this.visual && this.mode !== 'insert' && this.mode !== 'replace') {
      if (selection.main.empty) { this.visual = null; this.mode = 'normal'; }
      else this.visual = { anchor: selection.main.from, head: prevGrapheme(this.host.state().doc, selection.main.to) };
    }
  }

  private announceMode(): void { this.host.announce(labels[this.mode]); this.refresh(); }

  private message(message: string, kind: 'info' | 'error' = 'info'): void {
    this.snapshotValue.message = message || null; this.snapshotValue.messageKind = message ? kind : null;
    if (message) this.host.announce(message, kind === 'error');
    this.refresh();
  }

  private refresh(): void {
    if (this.disposed) return;
    const state = this.host.state(), cursor = Math.min(this.visual ? this.visual.head : this.mode === 'normal' ? clampNormal(state.doc, state.selection.main.head) : state.selection.main.head, state.doc.length);
    const shape = this.mode === 'insert' ? 'bar' : this.mode === 'replace' || this.mode === 'operator-pending' ? 'underline' : 'block';
    const next = this.mode === 'insert' ? cursor : Math.min(nextGrapheme(state.doc, cursor), state.doc.lineAt(cursor).to);
    this.snapshotValue = { ...this.snapshotValue, mode: this.mode, label: labels[this.mode], pending: this.parser.pending(), recording: this.macro.register, cursorShape: shape, displayCursor: cursor, displayCursorEnd: next };
    const snapshot = this.snapshot;
    for (const listener of [...this.listeners]) listener(snapshot);
  }

}

import { runMotion as importMotion } from './motions';
