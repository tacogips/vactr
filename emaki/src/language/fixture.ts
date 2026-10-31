export function generateJsonFixture({ lines, seed = 1 }: { lines: number; seed?: number }): string {
  if (!Number.isInteger(lines) || lines < 3) throw new RangeError('lines must be an integer of at least 3');
  let value = seed >>> 0;
  const random = () => { value = (value + 0x6d2b79f5) >>> 0; let t = value; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const out = ['[']; let id = 0; let remaining = lines - 2;
  while (remaining >= 10) {
    const count = Math.min(1 + Math.floor(random() * 4), remaining - 9);
    const tags = Array.from({ length: count }, (_, n) => `      ${JSON.stringify(`tag-${id}-${n}-deterministic-category-label`)}${n + 1 < count ? ',' : ''}`);
    const record = ['  {', `    "id": ${id},`, `    "name": ${JSON.stringify(`item-${id}-deterministic-readable-record-name ${id % 5 === 0 ? '"escaped" \\ path' : 'sample'}`)},`, '    "tags": [', ...tags, '    ],', `    "active": ${random() < 0.5},`, `    "score": ${(random() * 1000).toFixed(3)},`, `    "parent": ${id % 3 === 0 ? 'null' : Math.floor(random() * (id + 1))},`, `    "meta": { "group": ${id % 17}, "note": "sample metadata for a reproducible document" }`, '  }'];
    if (record.length > remaining) break;
    if (id > 0) out[out.length - 1] += ',';
    out.push(...record); remaining -= record.length; id++;
  }
  if (remaining > 0) {
    if (id > 0) { const at = out.length - 1; out[at - 1] += ','; out.splice(at, 0, ...Array.from({ length: remaining }, (_, n) => `    \"extra${n}\": null${n + 1 < remaining ? ',' : ''}`)); }
    else out.push(...Array.from({ length: remaining }, () => ''));
  }
  out.push(']'); return out.join('\n');
}
