import type { CodeSurface } from '../app/apis';
import type { Timers } from '../protocol/document';
import { defaultTimers } from '../protocol/document';
import type { DraftStore } from './types';
import type { DocumentSession } from './session';

export interface DraftAutosaveOptions {
  surface: CodeSurface;
  session: DocumentSession;
  drafts: DraftStore;
  win: Window | null;
  timers?: Timers;
  now?: () => number;
  delayMs?: number;
  notice(message: string): void;
}

/** Debounced, dirty-only web draft persistence for a DocumentSession. */
export class DraftAutosave {
  private readonly timers: Timers;
  private readonly now: () => number;
  private readonly delayMs: number;
  private timer: unknown = null;
  private noticeShown = false;
  private disposed = false;
  private readonly stopSurface: () => void;
  private readonly stopBaseline: () => void;
  private readonly onPageHide = () => { if (this.timer !== null) this.flush(); };
  private readonly onVisibilityChange = () => {
    if (this.opts.win?.document.visibilityState === 'hidden' && this.timer !== null) this.flush();
  };

  constructor(private readonly opts: DraftAutosaveOptions) {
    this.timers = opts.timers ?? defaultTimers;
    this.now = opts.now ?? Date.now;
    this.delayMs = opts.delayMs ?? 1000;
    this.stopSurface = opts.surface.subscribe((update) => {
      if (update.docChanged) this.arm();
    });
    this.stopBaseline = opts.session.onBaseline(() => {
      this.cancel();
      if (!opts.session.dirty()) opts.drafts.clearDraft();
    });
    opts.win?.addEventListener('pagehide', this.onPageHide);
    opts.win?.document.addEventListener('visibilitychange', this.onVisibilityChange);
  }

  flush(): void {
    if (this.disposed) return;
    this.cancel();
    if (!this.opts.session.dirty()) {
      this.opts.drafts.clearDraft();
      return;
    }
    const result = this.opts.drafts.writeDraft({
      text: this.opts.surface.state.doc.toString(),
      source: this.opts.session.source,
      savedAt: this.now(),
    });
    if (result.ok) {
      this.noticeShown = false;
      return;
    }
    if (result.reason === 'too-large' && !this.noticeShown) {
      this.opts.notice('Document too large for the browser draft; use Save file');
      this.noticeShown = true;
    } else if (result.reason === 'quota' && !this.noticeShown) {
      this.opts.notice('Browser storage is full; draft not saved');
      this.noticeShown = true;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.cancel();
    this.stopSurface();
    this.stopBaseline();
    this.opts.win?.removeEventListener('pagehide', this.onPageHide);
    this.opts.win?.document.removeEventListener('visibilitychange', this.onVisibilityChange);
  }

  private arm(): void {
    this.cancel();
    this.timer = this.timers.set(() => {
      this.timer = null;
      this.flush();
    }, this.delayMs);
  }

  private cancel(): void {
    if (this.timer !== null) {
      this.timers.clear(this.timer);
      this.timer = null;
    }
  }
}
