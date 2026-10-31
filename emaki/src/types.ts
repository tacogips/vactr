import type { ChangeSet, EditorState } from '@codemirror/state';
import type { Palette } from './render/palette';

export type Rgba = [number, number, number, number];
export interface Rect { left: number; right: number; top: number; bottom: number }
export interface Range { from: number; to: number }
export type CursorShape = 'bar' | 'block' | 'underline';
export interface ModalKeyInput { key: string; code?: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }
export interface ModalKeyHook { handleKey(input: ModalKeyInput): boolean; allowsTextInput(): boolean; overwriteRange(text: string): Range | null; noteCompositionDiscarded(): void; flushPendingInput?(): void }
export interface MarkStyle {
  color?: Rgba; background?: Rgba; fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic';
  underline?: { color: Rgba; thickness: 1 | 2 | 3 }; strike?: { color: Rgba }; opacity?: number;
  glow?: { color: Rgba; radius: number }; label?: { text: string; fill?: Rgba; color?: Rgba }; minWidth?: number;
}
export type AnimatedMarkStyle = Pick<MarkStyle, 'background' | 'glow' | 'underline' | 'strike' | 'label' | 'minWidth'>;
export interface Mark extends Range { style: MarkStyle }
export interface AnimatedMark extends Range { style: AnimatedMarkStyle }
export interface LineFormatStyle {
  background?: Rgba; color?: Rgba; fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic'; opacity?: number;
  bar?: { color: Rgba; width: number }; gutterMarker?: { text: string; color: Rgba }; gutterBackground?: Rgba;
}
export interface LineFormat { pos: number; style: LineFormatStyle }
export interface ResolvedLineFormat { line: number; style: LineFormatStyle }
export interface TokenSpan extends Range { tag: string }
export interface LineToken { from: number; to: number; tag: string }
export type WidgetContent = { kind: 'dom'; mount(el: HTMLElement): () => void } | { kind: 'canvas'; draw(ctx: CanvasRenderingContext2D, info: { width: number; height: number; dpr: number; frameMs: number }): boolean };
export interface BlockWidgetSpec { key: string; pos: number; side: 'above' | 'below'; height: number; content: WidgetContent; interactive?: boolean; label?: string }
export interface EolWidgetSpec { key: string; pos: number; width: number; content: WidgetContent; interactive?: boolean; label?: string }
export interface LineLanguage<S> { kind: 'lines'; name: string; startState(): S; tokenizeLine(text: string, state: S): { tokens: readonly LineToken[]; state: S }; sameState(a: S, b: S): boolean }
export interface SpanLanguage { kind: 'spans'; name: string; spans(state: EditorState, from: number, to: number, limit: number): { spans: readonly TokenSpan[]; truncated: boolean }; noteChanges?(changes: ChangeSet, state: EditorState): void; setInvalidate?(cb: () => void): void; dispose?(): void }
export type LanguageProvider = LineLanguage<unknown> | SpanLanguage;
export interface PluginViewport { fromLine: number; toLine: number; top: number; height: number }
export interface PluginUpdate { state: EditorState; changes: ChangeSet; docChanged: boolean; selectionSet: boolean }
export interface PluginInstance { docChanged?(u: PluginUpdate): void; viewportChanged?(v: PluginViewport): void; frame?(frameMs: number): boolean; destroy?(): void }
export interface PluginHost {
  readonly state: EditorState;
  marks(owner: string): { set(marks: readonly Mark[]): void; clear(): void };
  animatedMarks(provider: (frameMs: number) => readonly AnimatedMark[]): () => void;
  lineFormats(owner: string): { set(formats: readonly LineFormat[]): void; clear(): void };
  widgets(owner: string): { set(blocks: readonly BlockWidgetSpec[], eol?: readonly EolWidgetSpec[]): void; clear(): void };
  requestFrame(): void; setActive(active: boolean): void;
  coordsAtPos(pos: number): Rect | null; lineTop(line: number): number;
  readonly viewport: PluginViewport; readonly theme: { palette: Palette; tokens: Readonly<Record<string, Rgba>> };
  reportError(error: unknown): void;
}
export interface EmakiPlugin { name: string; attach(host: PluginHost): PluginInstance | void }
