import type { ShapedLine } from '../layout/layout';
import type { TokenSpan, Mark, ResolvedLineFormat, Rgba } from '../types';

export interface TextDecoration { from: number; to: number; color?: Rgba; opacity?: number; fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic' }
export interface DecorationIndex {
  marks: Array<{ mark: Mark; order: number; maxTo: number }>;
  lines: Map<number, ResolvedLineFormat[]>;
  key: string;
}
export function indexTextDecorations(marks: readonly Mark[], formats: readonly ResolvedLineFormat[]): DecorationIndex {
  const sorted = marks.filter(mark => mark.style.color !== undefined || mark.style.opacity !== undefined || mark.style.fontWeight !== undefined || mark.style.fontStyle !== undefined)
    .map((mark, order) => ({ mark, order, maxTo: mark.to }))
    .sort((a, b) => a.mark.from - b.mark.from || a.order - b.order);
  let maxTo = -1;
  for (const item of sorted) { maxTo = Math.max(maxTo, item.mark.to); item.maxTo = maxTo; }
  const lines = new Map<number, ResolvedLineFormat[]>();
  for (const format of formats) {
    if (format.style.color === undefined && format.style.opacity === undefined && format.style.fontWeight === undefined && format.style.fontStyle === undefined) continue;
    const list = lines.get(format.line) ?? []; list.push(format); lines.set(format.line, list);
  }
  const key = JSON.stringify([sorted.map(({ mark, order }) => [order, mark.from, mark.to, mark.style.color, mark.style.opacity, mark.style.fontWeight, mark.style.fontStyle]),
    [...lines].map(([line, list]) => [line, list.map(({ style }) => [style.color, style.opacity, style.fontWeight, style.fontStyle])])]);
  return { marks: sorted, lines, key };
}
export function lineDecorations(index: DecorationIndex, line: ShapedLine): TextDecoration[] {
  let lo = 0, hi = index.marks.length;
  while (lo < hi) { const mid = (lo + hi) >>> 1; if (index.marks[mid]!.maxTo <= line.from) lo = mid + 1; else hi = mid; }
  const candidates: Array<{ mark: Mark; order: number }> = [];
  for (let i = lo; i < index.marks.length && index.marks[i]!.mark.from < line.to; i++) {
    const item = index.marks[i]!; if (item.mark.to > line.from) candidates.push(item);
  }
  candidates.sort((a, b) => a.order - b.order);
  let lineColor: Rgba | undefined, lineOpacity: number | undefined;
  let fontWeight: 'normal' | 'bold' | undefined, fontStyle: 'normal' | 'italic' | undefined;
  for (const format of index.lines.get(line.number) ?? []) {
    lineColor = format.style.color ?? lineColor; lineOpacity = format.style.opacity ?? lineOpacity;
    fontWeight = format.style.fontWeight ?? fontWeight; fontStyle = format.style.fontStyle ?? fontStyle;
  }
  const result: TextDecoration[] = [{ from: line.from, to: line.to, color: lineColor, opacity: lineOpacity, fontWeight, fontStyle }];
  for (const { mark } of candidates) result.push({ from: mark.from, to: mark.to, color: mark.style.color, opacity: mark.style.opacity, fontWeight: mark.style.fontWeight, fontStyle: mark.style.fontStyle });
  return result;
}
export function fontVariantsForLine(layout: TextLayout, line: ShapedLine, decorations: readonly TextDecoration[]): import('../layout/layout').FontVariantRange[] {
  const edges = new Set<number>([line.from, line.to]);
  for (const d of decorations) if (d.fontWeight !== undefined || d.fontStyle !== undefined) {
    edges.add(layout.boundary(Math.max(line.from, d.from), -1));
    edges.add(layout.boundary(Math.min(line.to, d.to), 1));
  }
  const sorted = [...edges].sort((a, b) => a - b), ranges: import('../layout/layout').FontVariantRange[] = [];
  for (let i = 0; i + 1 < sorted.length; i++) {
    const from = sorted[i]!, to = sorted[i + 1]!; let weight = 'normal', style = 'normal';
    for (const d of decorations) if (d.from < to && d.to > from) { weight = d.fontWeight ?? weight; style = d.fontStyle ?? style; }
    const variant = [weight === 'bold' ? 'bold' : '', style === 'italic' ? 'italic' : ''].filter(Boolean).join(' ') || 'normal';
    if (variant !== 'normal') ranges.push({ from, to, variant });
  }
  return ranges;
}
function tintAt(base: number[], decorations: readonly TextDecoration[], from: number, to: number): number[] {
  let color = base, lineOpacity = 1, markOpacity = 1;
  decorations.forEach((decoration, index) => {
    if (decoration.from >= to || decoration.to <= from) return;
    if (decoration.color) color = decoration.color;
    if (decoration.opacity !== undefined) { if (index === 0) lineOpacity = decoration.opacity; else markOpacity = decoration.opacity; }
  });
  return [color[0]!, color[1]!, color[2]!, color[3]! * lineOpacity * markOpacity];
}

export function tokenKey(tokens: readonly TokenSpan[]): string {
  return tokens.map(token => `${token.from}:${token.to}:${token.tag}`).join(';');
}
export function tokenStyleHash(line: ShapedLine, tokens: readonly TokenSpan[], from: number, to: number): string {
  return tokens.filter(token => token.from < to && token.to > from)
    .map(token => `${Math.max(0, token.from - line.from)}:${Math.min(line.to, token.to) - line.from}:${token.tag}`).join(';');
}

import type { TextLayout, LayoutViewport, ShapedRun } from '../layout/layout';
import type { Rect } from '../types';
import { rgbaCss, type Palette } from './palette';
import { CLIP_GUTTER, SNAP_GLYPH, UNTINTED, glyphPhase, glyphQuad, type GeometryLayer, type InstanceWriter, type LayerBuffer } from './geometry';
import { MAX_TEXT_BLOCKS, type SegmentCache, type SegmentKey } from './segments';
import type { GlyphAtlas } from './atlas';

export interface DrawCommand { rect: Rect; color: number[]; texture?: WebGLTexture; uv?: [number, number, number, number]; generation?: number; cursor?: boolean; flags?: number; layer: GeometryLayer; slot: number }
export interface TextPassContext {
  layout: TextLayout; view: LayoutViewport; atlas: GlyphAtlas | null; gl: WebGL2RenderingContext | null;
  maxTextureSize: number; slotTexture: WebGLTexture | null; scale: number; textureGeneration: number;
  lineSlots: WeakMap<object, number>; slotOwners: Map<number, object>; liveSlots: Map<number, string>; freeSlots: number[]; nextSlot: number;
  gutterIdentities: Map<number, readonly ShapedRun[]>; lineNumbersByRuns: WeakMap<object, number>; shiftedGutterIdentities: WeakMap<object, readonly ShapedRun[]>;
  segments: SegmentCache<DrawCommand[]>; pendingSegments: Set<string>; layerBuffers: Map<GeometryLayer, LayerBuffer>; writers: Map<GeometryLayer, InstanceWriter>;
  stats: { slotTableBytes: number; compactions: number; gutterBuilds: number; textBuilds: number; bufferUploads: number; geometryBytes: number; decodedTextInstances: number };
  visibleLines: ShapedLine[]; pendingText: boolean; currentSegmentPending: boolean; pendingStartLine: number; currentLineIndex: number;
  currentSlot: number; currentLayer: GeometryLayer; collecting: DrawCommand[] | null; textLayerDirty: boolean;
  decorationIndex: DecorationIndex; textHighWaterBlocks: number; uploadedText: Uint8Array; textCommands: DrawCommand[] | null; palette: Palette;
  writeSlots(lines: readonly ShapedLine[]): void; styleHash(line: ShapedLine, styles: readonly TokenSpan[], from?: number, to?: number): string;
  gutterIdentity(line: number): readonly ShapedRun[]; labelRun(text: string, width: number): ShapedRun;
  drawRun(run: ShapedRun, x: number, y: number, styles: readonly TokenSpan[], color?: string, gutter?: boolean, decorations?: readonly TextDecoration[]): void;
  quad(rect: Rect, color: number[], texture?: WebGLTexture | null, uv?: [number, number, number, number], flags?: number): void;
}

export function writeSlots(this: TextPassContext, lines: readonly ShapedLine[]): void {
    if (lines.length + 1 > this.maxTextureSize) throw new Error('Visible line slot table exceeds texture limit');
    const active = new Set<number>(), changed = new Map<number, [number, number]>();
    for (const line of lines) {
      const identity = line.runs as object;
      let slot = this.lineSlots.get(identity);
      if (slot !== undefined && this.slotOwners.get(slot) !== identity) {
        this.lineSlots.delete(identity); slot = undefined;
      }
      if (slot === undefined) {
        while (slot === undefined) {
          const available = this.freeSlots.pop();
          if (available === undefined) slot = this.nextSlot++;
          else if (!this.slotOwners.has(available)) slot = available;
        }
        this.lineSlots.set(identity, slot); this.slotOwners.set(slot, identity);
      }
      active.add(slot);
      if (slot >= Math.min(1024, this.maxTextureSize)) { this.pendingText = true; continue; }
      const x = -this.view.scrollLeft, y = this.layout.heightmap.lineTop(line.number) - this.view.scrollTop;
      const position = `${x}:${y}`;
      if (this.liveSlots.get(slot) !== position) {
        changed.set(slot, [x, y]);
        this.liveSlots.set(slot, position);
      }
    }
    for (const slot of this.slotOwners.keys()) if (!active.has(slot)) {
      if (this.liveSlots.has(slot)) changed.set(slot, [0, 0]);
      this.liveSlots.delete(slot);
      const owner = this.slotOwners.get(slot);
      if (owner) this.lineSlots.delete(owner);
      this.slotOwners.delete(slot); this.freeSlots.push(slot);
    }
    const slots = [...changed.keys()].sort((a, b) => a - b);
    const gl = this.gl!; gl.activeTexture(gl.TEXTURE1); gl.bindTexture(gl.TEXTURE_2D, this.slotTexture);
    for (let at = 0; at < slots.length;) {
      const start = slots[at]!; let end = at + 1;
      while (end < slots.length && slots[end] === slots[end - 1]! + 1) end++;
      const values = new Float32Array((end - at) * 2);
      for (let index = at; index < end; index++) values.set(changed.get(slots[index]!)!, (index - at) * 2);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, start, 0, end - at, 1, gl.RG, gl.FLOAT, values);
      this.stats.slotTableBytes += values.byteLength; at = end;
    }
  }
export function styleHash(this: TextPassContext, line: ShapedLine, styles: readonly TokenSpan[], from = line.from, to = line.to): string {
    return tokenStyleHash(line, styles, from, to);
  }
export function gutterIdentity(this: TextPassContext, lineNumber: number): readonly ShapedRun[] {
    let identity = this.gutterIdentities.get(lineNumber);
    if (!identity) { identity = []; this.gutterIdentities.set(lineNumber, identity); }
    return identity;
  }
export function rebuildTextSegments(this: TextPassContext, styles: readonly TokenSpan[]): void {
    const visibleFirst = this.layout.heightmap.lineAtHeight(this.view.scrollTop).line;
    // Exclusive end: a line whose text top sits exactly on the bottom edge is not visible.
    const viewBottom = this.view.scrollTop + this.view.height, bottomHit = this.layout.heightmap.lineAtHeight(viewBottom).line;
    const visibleLast = bottomHit + (this.layout.heightmap.lineTop(bottomHit) < viewBottom ? 1 : 0);
    const rows = this.visibleLines.map(line => {
      const runs = line.runs, identity = runs as object;
      const decorations = lineDecorations(this.decorationIndex, line);
      const decorationHash = JSON.stringify(decorations.map(d => [d.from - line.from, d.to - line.from, d.color, d.opacity]));
      const previousNumber = this.lineNumbersByRuns.get(identity);
      this.lineNumbersByRuns.set(identity, line.number);
      const chunks = this.layout.geometrySegments(line).filter(chunk => {
        const first = chunk.runs[0], last = chunk.runs[chunk.runs.length - 1];
        const from = first?.x ?? 0, to = last ? last.x + last.width : 0;
        return to >= Math.max(0, this.view.scrollLeft - this.view.width) && from <= this.view.scrollLeft + this.view.width * 2;
      });
      const textKeys = chunks.map(chunk => {
        const from = chunk.runs[0]?.from ?? line.from, last = chunk.runs[chunk.runs.length - 1];
        const to = last?.to ?? line.to;
        return { chunk, key: { runs, chunk: chunk.chunk, fontGeneration: this.layout.font.generation ?? 0,
          dpr: this.scale, styleHash: this.styleHash(line, styles, from, to) + decorationHash, atlasGeneration: this.textureGeneration } as SegmentKey };
      });
      let shiftedGutter = this.shiftedGutterIdentities.get(identity);
      if (!shiftedGutter && previousNumber !== undefined && previousNumber !== line.number) {
        shiftedGutter = runs; this.shiftedGutterIdentities.set(identity, shiftedGutter);
      }
      const gutterRuns = shiftedGutter ?? this.gutterIdentity(line.number);
      const gutterKey: SegmentKey = { runs: gutterRuns, chunk: -1, fontGeneration: this.layout.font.generation ?? 0,
        dpr: this.scale, styleHash: `gutter:${line.number + 1}`, atlasGeneration: this.textureGeneration };
      return { line, identity, textKeys, gutterKey, decorations, canBuild: line.number >= visibleFirst && line.number < visibleLast };
    });
    const visibleNumbers = new Set(rows.map(row => row.line.number));
    for (const number of this.gutterIdentities.keys()) if (!visibleNumbers.has(number)) this.gutterIdentities.delete(number);
    const keep = new Set<string>();
    for (const row of rows) { for (const { key } of row.textKeys) keep.add(this.segments.key(key)); keep.add(this.segments.key(row.gutterKey)); }
    this.writeSlots(this.visibleLines);
    const removed = this.segments.deleteOutside(keep);
    if (removed.length) this.textLayerDirty = true;
    const used = this.segments.usedBlocks;
    if (used > 0 && this.segments.highWaterBlocks > 2 * used) {
      this.segments.compact(); this.stats.compactions++; this.uploadedText = new Uint8Array();
    }
    const writer = this.writers.get('text')!; writer.clear();
    for (let offset = 0; offset < rows.length; offset++) {
      const index = (this.pendingStartLine + offset) % Math.max(1, rows.length), { line, identity, textKeys, gutterKey, decorations, canBuild } = rows[index]!;
      const gutterId = this.segments.key(gutterKey);
      if (this.pendingSegments.delete(gutterId)) this.segments.delete(gutterKey);
      let gutter = (this.view.gutter ?? 48) > 0 ? this.segments.get(gutterKey) : undefined;
      if (!gutter && canBuild && (this.view.gutter ?? 48) > 0) {
        this.currentSlot = this.lineSlots.get(identity)!; this.currentLayer = 'text'; this.collecting = []; this.currentSegmentPending = false;
        this.drawRun(this.labelRun(String(line.number + 1), (this.view.gutter ?? 48) - 8), 4, 0, [], rgbaCss(this.palette.gutter), true);
        gutter = this.segments.set(gutterKey, this.collecting, 0) ?? undefined;
        if (!gutter) this.pendingText = true;
        else { this.stats.gutterBuilds++; if (this.currentSegmentPending) this.pendingSegments.add(gutterId); }
        this.currentSlot = 0;
      }
      let gutterPacked = false;
      for (let chunkIndex = 0; chunkIndex < textKeys.length; chunkIndex++) {
        const { chunk, key: textKey } = textKeys[chunkIndex]!;
        const textId = this.segments.key(textKey);
        if (this.pendingSegments.delete(textId)) this.segments.delete(textKey);
        let segment = this.segments.get(textKey);
        if (!segment && canBuild) {
          this.currentLineIndex = index; this.currentSlot = this.lineSlots.get(identity)!;
          this.currentLayer = 'text'; this.collecting = []; this.currentSegmentPending = false;
          const oldWidth = this.view.width;
          this.view.width = Math.max(oldWidth, (this.view.gutter ?? 48) + line.width + 8);
          const from = chunk.runs[0]?.from ?? line.from, last = chunk.runs[chunk.runs.length - 1];
          const to = last?.to ?? line.to;
          const chunkStyles = styles.filter(style => style.from < to && style.to > from);
          for (const run of chunk.runs) this.drawRun(run, (this.view.gutter ?? 48) + run.x, 0, chunkStyles, undefined, false, decorations);
          this.view.width = oldWidth;
          const built = this.segments.set(textKey, this.collecting, 0);
          this.collecting = null;
          this.currentSlot = 0;
          if (!built) this.pendingText = true;
          segment = built ?? undefined;
          if (built) {
            this.stats.textBuilds++;
            if (this.currentSegmentPending) this.pendingSegments.add(textId);
          }
        }
        if (!segment) continue;
        const commands = chunkIndex === 0 ? [...segment.value, ...(gutter?.value ?? [])] : segment.value;
        if (!this.segments.ensureBlocks(textKey, Math.max(1, Math.ceil(commands.length / 64)))) { this.pendingText = true; continue; }
        let wroteAllCommands = true;
        for (let commandIndex = 0; commandIndex < commands.length; commandIndex++) {
          const command = commands[commandIndex]!;
          const block = segment.blocks[Math.floor(commandIndex / 64)];
          if (block === undefined) { this.pendingText = true; wroteAllCommands = false; break; }
          const gutterCommand = chunkIndex === 0 && commandIndex >= segment.value.length;
          const rect = gutterCommand
            ? { ...command.rect, left: command.rect.left + this.view.scrollLeft, right: command.rect.right + this.view.scrollLeft }
            : command.rect;
          writer.pushAt(block * 64 + commandIndex % 64, rect,
            command.uv ?? [0, 0, 2 / this.atlas!.dimensions[0], 2 / this.atlas!.dimensions[1]],
            command.color as [number, number, number, number], this.lineSlots.get(identity)!, command.flags);
        }
        if (chunkIndex === 0 && gutter && wroteAllCommands) gutterPacked = true;
      }
      if (gutter && gutter.value.length > 0 && !gutterPacked) {
        if (!this.segments.ensureBlocks(gutterKey, Math.max(1, Math.ceil(gutter.value.length / 64)))) this.pendingText = true;
        else for (let commandIndex = 0; commandIndex < gutter.value.length; commandIndex++) {
          const command = gutter.value[commandIndex]!;
          const block = gutter.blocks[Math.floor(commandIndex / 64)];
          if (block === undefined) { this.pendingText = true; break; }
          const rect = { ...command.rect, left: command.rect.left + this.view.scrollLeft, right: command.rect.right + this.view.scrollLeft };
          writer.pushAt(block * 64 + commandIndex % 64, rect,
            command.uv ?? [0, 0, 2 / this.atlas!.dimensions[0], 2 / this.atlas!.dimensions[1]],
            command.color as [number, number, number, number], this.lineSlots.get(identity)!, command.flags);
        }
      }
      this.collecting = null;
    }
    const blocks = Math.min(MAX_TEXT_BLOCKS, this.segments.highWaterBlocks);
    writer.setCount(blocks * 64); this.textHighWaterBlocks = blocks;
    const next = writer.bytes();
    const previous = this.uploadedText;
    const layer = this.layerBuffers.get('text')!;
    const totalBlocks = Math.max(Math.ceil(previous.length / 2048), Math.ceil(next.length / 2048));
    let block = 0;
    while (block < totalBlocks) {
      const offset = block * 2048;
      const a = previous.subarray(offset, offset + 2048), b = next.subarray(offset, offset + 2048);
      if (a.length === b.length && a.every((value, i) => value === b[i])) { block++; continue; }
      const start = block++;
      while (block < totalBlocks) {
        const nextOffset = block * 2048, before = previous.subarray(nextOffset, nextOffset + 2048), after = next.subarray(nextOffset, nextOffset + 2048);
        if (before.length === after.length && before.every((value, i) => value === after[i])) break;
        block++;
      }
      const end = block * 2048, data = new Uint8Array(end - start * 2048);
      data.set(next.subarray(start * 2048, end));
      layer.uploadRange(start * 2048, data); this.stats.bufferUploads++; this.stats.geometryBytes += data.byteLength; this.textLayerDirty = true;
    }
    this.uploadedText = next.slice();
    this.textCommands = [];
    this.stats.decodedTextInstances = writer.count;
    if (!this.pendingText) this.textLayerDirty = false;
  }
export function drawRun(this: TextPassContext, run: ShapedRun, x: number, y: number, styles: readonly TokenSpan[], color?: string, gutter = false, decorations: readonly TextDecoration[] = []): void {
    const atlas = this.atlas!; const pad = Math.ceil(2 * this.scale); const step = (atlas.maxCellWidth - 2 * pad) / this.scale;
    const left = gutter ? 0 : this.view.gutter ?? 48; const right = gutter ? this.view.gutter ?? 48 : this.view.width;
    if (!this.layout.isRunLine(run)) {
      for (const cluster of this.layout.textCells(run, Math.max(0, left - x - 8), right - x + 8)) {
        const glyphX = x + cluster.x;
        if (glyphX + cluster.width < left || glyphX > right) continue;
        const from = run.from + cluster.from, to = run.from + cluster.to;
        const style = styles.find(a => a.from < to && a.to > from);
        const tint = color ?? (style ? rgbaCss(this.palette.token[style.tag] ?? this.palette.text) : rgbaCss(this.palette.text));
        const emoji = /\p{Extended_Pictographic}/u.test(cluster.text);
        const resolvedTint = tintAt(parseCssColor(tint), decorations, from, to);
        const { devicePixel, phase } = glyphPhase(glyphX, this.scale);
        const font = run.variant && run.variant !== 'normal' ? { ...this.layout.font, font: `${run.variant} ${this.layout.font.font}` } : this.layout.font;
        const cell = atlas.cell({ kind: emoji ? 'color' : 'mask', text: cluster.text, font, dpr: this.scale, width: cluster.width, phase, ...(emoji ? { opacity: resolvedTint[3] } : {}) });
        if (!cell) {
          if (!this.pendingText) this.pendingStartLine = (this.currentLineIndex + 1) % Math.max(1, this.visibleLines.length);
          this.pendingText = true; this.currentSegmentPending = true; continue;
        }
        const flags = (gutter ? 0 : CLIP_GUTTER) | (emoji ? UNTINTED : 0) | SNAP_GLYPH;
        const quad = glyphQuad(devicePixel, this.scale, cell);
        this.quad({ left: quad.left, right: quad.left + quad.width, top: y, bottom: y + quad.height }, resolvedTint, atlas.texture,
          [cell.u0, cell.v0, cell.u1, cell.v1], flags);
      }
      return;
    }
    const runStyles = color ? [] : styles.filter(a => a.from < run.to && a.to > run.from);
    const boundaries = [0, run.width];
    for (const style of runStyles) {
      boundaries.push(this.layout.runOffset(run, Math.max(0, style.from - run.from)));
      boundaries.push(this.layout.runOffset(run, Math.min(run.text.length, style.to - run.from)));
    }
    for (const decoration of decorations) {
      if (decoration.from < run.to && decoration.to > run.from) {
        boundaries.push(this.layout.runOffset(run, Math.max(0, decoration.from - run.from)));
        boundaries.push(this.layout.runOffset(run, Math.min(run.text.length, decoration.to - run.from)));
      }
    }
    boundaries.sort((a, b) => a - b);
    const edges = boundaries.filter((value, index) => index === 0 || value > boundaries[index - 1]!);
    for (let segment = 0; segment < edges.length - 1; segment++) {
      const segmentFrom = edges[segment]!, segmentTo = edges[segment + 1]!;
      if (segmentTo <= segmentFrom) continue;
      const middle = (segmentFrom + segmentTo) / 2;
      const at = this.layout.offsetInRun(run, middle, 1);
      const style = runStyles.find(a => a.from <= at && a.to > at);
      const tint = color ?? (style ? rgbaCss(this.palette.token[style.tag] ?? this.palette.text) : rgbaCss(this.palette.text));
      let offset = Math.max(segmentFrom, left - x); const visibleTo = Math.min(segmentTo, right - x);
      while (offset < visibleTo) {
        const width = Math.min(step, segmentTo - offset, visibleTo - offset);
        if (width <= 0) break;
        const { devicePixel, phase } = glyphPhase(x + offset, this.scale);
        const font = run.variant && run.variant !== 'normal' ? { ...this.layout.font, font: `${run.variant} ${this.layout.font.font}` } : this.layout.font;
        const cell = atlas.cell({ kind: 'run', text: run.text, run, piece: { from: offset, to: offset + width }, chunkX: offset, font, dpr: this.scale, width, phase });
        if (!cell) { if (!this.pendingText) this.pendingStartLine = (this.currentLineIndex + 1) % Math.max(1, this.visibleLines.length); this.pendingText = true; this.currentSegmentPending = true; offset += width; continue; }
        const flags = (gutter ? 0 : CLIP_GUTTER) | SNAP_GLYPH;
        const quad = glyphQuad(devicePixel, this.scale, cell);
        this.quad({ left: quad.left, right: quad.left + quad.width, top: y, bottom: y + quad.height }, tintAt(parseCssColor(tint), decorations, at, at + 1), atlas.texture,
          [cell.u0, cell.v0, cell.u1, cell.v1], flags);
        offset += width;
      }
    }
  }

function parseCssColor(value: string): number[] {
  const hex = /^#([\da-f]{6})$/i.exec(value);
  if (hex) return [0, 2, 4].map((index) => Number.parseInt(hex[1]!.slice(index, index + 2), 16) / 255).concat(1);
  const rgba = /^rgba?\(([^)]+)\)$/.exec(value);
  if (rgba) {
    const parts = rgba[1]!.split(',').map((part) => Number(part.trim()));
    if (parts.length >= 3 && parts.slice(0, 3).every(Number.isFinite)) return [parts[0]! / 255, parts[1]! / 255, parts[2]! / 255, parts[3] ?? 1];
  }
  return [1, 1, 1, 1];
}

