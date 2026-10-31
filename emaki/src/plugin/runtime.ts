import type { EditorState } from '@codemirror/state';
import type { DecorationStore } from '../decor/store';
import type { EmakiPlugin, PluginHost, PluginInstance, PluginUpdate, PluginViewport, Rect } from '../types';
import type { WidgetSet } from './widgets';

interface Options {
  store: DecorationStore; widgets: WidgetSet; getState(): EditorState; requestFrame(): void;
  coordsAtPos(pos: number): Rect | null; lineTop(line: number): number;
  viewport(): PluginViewport; theme(): PluginHost['theme']; onError?(plugin: string, error: unknown): void;
}
interface Entry { instance?: PluginInstance; active: boolean; remove(): void; report(error: unknown): void }

export class PluginRuntime {
  private entries: Entry[] = [];
  private nextOwnerNamespace = 0;
  private lastViewport: PluginViewport | undefined;
  constructor(private readonly options: Options) {}
  add(plugin: EmakiPlugin): () => void {
    const owners = new Set<string>(); const animated = new Set<() => void>();
    const namespace = this.nextOwnerNamespace++;
    let removed = false; let reported = false;
    const entry: Entry = { active: false, report: error => report(error), remove: () => {
      if (removed) return; removed = true;
      this.entries = this.entries.filter(item => item !== entry);
      try { entry.instance?.destroy?.(); } catch (error) { report(error); }
      for (const owner of owners) { this.options.store.marks(owner).clear(); this.options.store.lineFormats(owner).clear(); this.options.widgets.clear(owner); }
      for (const dispose of animated) dispose(); animated.clear(); this.options.requestFrame();
    } };
    const report = (error: unknown): void => { if (!reported) { reported = true; this.options.onError?.(plugin.name, error); } };
    const owner = (name: string): string => { const key = `${namespace}:${plugin.name}:${name}`; owners.add(key); return key; };
    const options = this.options;
    const host: PluginHost = {
      get state() { return options.getState(); },
      marks: name => { const set = options.store.marks(owner(name)); return { set: marks => { set.set(marks); options.requestFrame(); }, clear: () => { set.clear(); options.requestFrame(); } }; },
      lineFormats: name => { const set = options.store.lineFormats(owner(name)); return { set: formats => { set.set(formats); options.requestFrame(); }, clear: () => { set.clear(); options.requestFrame(); } }; },
      widgets: name => { const key = owner(name); return { set: (blocks, eol) => { options.widgets.set(key, blocks, eol); options.requestFrame(); }, clear: () => { options.widgets.clear(key); options.requestFrame(); } }; },
      animatedMarks: provider => { const dispose = options.store.addAnimated(ms => { try { return provider(ms); } catch (error) { report(error); entry.remove(); return []; } }); animated.add(dispose); return () => { dispose(); animated.delete(dispose); }; },
      requestFrame: () => options.requestFrame(), setActive: active => { entry.active = active; if (active) options.requestFrame(); },
      coordsAtPos: pos => options.coordsAtPos(pos), lineTop: line => options.lineTop(line),
      get viewport() { return options.viewport(); }, get theme() { return options.theme(); }, reportError: report,
    };
    this.entries.push(entry);
    try { const instance = plugin.attach(host); if (instance) entry.instance = instance; } catch (error) { report(error); entry.remove(); }
    return entry.remove;
  }
  private call(entry: Entry, run: (instance: PluginInstance) => void): void {
    try { if (entry.instance) run(entry.instance); } catch (error) { entry.report(error); entry.remove(); }
  }
  docChanged(update: PluginUpdate): void {
    this.options.store.map(update.changes); this.options.widgets.map(update.changes);
    for (const entry of [...this.entries]) this.call(entry, instance => instance.docChanged?.(update));
  }
  viewportChanged(viewport: PluginViewport): void {
    const old = this.lastViewport;
    if (old && old.fromLine === viewport.fromLine && old.toLine === viewport.toLine && old.top === viewport.top && old.height === viewport.height) return;
    this.lastViewport = { ...viewport };
    for (const entry of [...this.entries]) this.call(entry, instance => instance.viewportChanged?.(viewport));
  }
  frame(frameMs: number): boolean {
    let active = false;
    for (const entry of [...this.entries]) { this.call(entry, instance => { if (instance.frame?.(frameMs)) active = true; }); if (entry.active) active = true; }
    return active;
  }
  destroy(): void { for (const entry of [...this.entries]) entry.remove(); }
}
