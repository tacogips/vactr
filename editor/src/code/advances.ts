import type { TextMetricsSource } from './layout';

const ASCII_START = 0x20;
const ASCII_END = 0x7e;
const PROBES = ['ffi', 'fi', 'fl', 'AV', 'To', '->', '==', 'www', 'Wa'];
const MAX_CLUSTERS = 8192;

export function createMeasureContext(doc: Document): TextMetricsSource | null {
  if (typeof OffscreenCanvas === 'function') {
    const context = new OffscreenCanvas(1, 1).getContext('2d');
    if (context) return context;
  }
  return doc.createElement('canvas').getContext('2d');
}

/** Advances measured from the same canvas font used to validate shaping. */
export class AdvanceTable {
  readonly ascii = new Float64Array(128);
  readonly stats = { builds: 0, measured: 0 };
  readonly additive: boolean;
  private readonly clusters = new Map<string, number>();

  constructor(private readonly measure: (text: string) => number) {
    for (let code = ASCII_START; code <= ASCII_END; code += 1) {
      this.ascii[code] = this.read(String.fromCharCode(code));
    }
    this.additive = PROBES.every((probe) => {
      const actual = this.read(probe);
      let sum = 0;
      for (let index = 0; index < probe.length; index += 1) sum += this.ascii[probe.charCodeAt(index)] ?? 0;
      return Math.abs(actual - sum) <= 0.01;
    });
    this.stats.builds = 1;
  }

  private read(text: string): number {
    this.stats.measured += 1;
    return this.measure(text);
  }

  cluster(text: string): number {
    if (text.length === 1 && text.charCodeAt(0) < 128) return this.ascii[text.charCodeAt(0)] ?? 0;
    const cached = this.clusters.get(text);
    if (cached !== undefined) {
      this.clusters.delete(text);
      this.clusters.set(text, cached);
      return cached;
    }
    const width = this.read(text);
    this.clusters.set(text, width);
    if (this.clusters.size > MAX_CLUSTERS) this.clusters.delete(this.clusters.keys().next().value!);
    return width;
  }

  width(text: string, clusters: number[] | null): number {
    let width = 0;
    let clusterIndex = 0;
    for (let index = 0; index < text.length;) {
      while (clusters && clusterIndex < clusters.length && clusters[clusterIndex + 1]! <= index) clusterIndex += 2;
      const start = index;
      if (text.charCodeAt(index) < 128) index += 1;
      else if (clusters && clusters[clusterIndex] === index) index = clusters[clusterIndex + 1]!;
      else index += text.codePointAt(index)! > 0xffff ? 2 : 1;
      width += this.cluster(text.slice(start, index));
    }
    return width;
  }

  chunks(text: string, clusters: number[] | null, maxChars: number): { start: number; end: number; width: number }[] {
    const result: { start: number; end: number; width: number }[] = [];
    let start = 0;
    while (start < text.length) {
      let end = Math.min(text.length, start + maxChars);
      if (end < text.length && clusters) {
        for (let index = 0; index < clusters.length; index += 2) {
          if (clusters[index]! < end && clusters[index + 1]! > end) { end = clusters[index]! > start ? clusters[index]! : clusters[index + 1]!; break; }
        }
      }
      if (end < text.length && end > 0) {
        const before = text.charCodeAt(end - 1), after = text.charCodeAt(end);
        if (before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff) end -= 1;
      }
      if (end <= start) end = Math.min(text.length, start + maxChars);
      const localClusters: number[] = [];
      if (clusters) for (let index = 0; index < clusters.length; index += 2) {
        if (clusters[index]! >= start && clusters[index + 1]! <= end) localClusters.push(clusters[index]! - start, clusters[index + 1]! - start);
      }
      result.push({ start, end, width: this.width(text.slice(start, end), localClusters) });
      start = end;
    }
    return result;
  }
}
