const encoder = new TextEncoder();
function mulberry32(seed) {
  return () => {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
export function createLargeDocument({ lines = 20_000, seed = 265 } = {}) {
  const random = mulberry32(seed);
  const synthDefinition = ['inst pad freq: float = 440 amp: float = 0.005:', '\tsaw freq', '\t\t> * amp'];
  const voices = Array.from({ length: 64 }, () => '{s :pad > note [60 64]}').join(' ');
  const head = [
    ...synthDefinition,
    `stack [${voices}] > d1`,
    'osc 20 > rotate 0.5 > out o0',
    'osc 20 > rotate 0.5 > out o1',
    'osc 20 > rotate 0.5 > out o2',
    'osc 20 > rotate 0.5 > out o3',
  ];
  const rows = [...head];
  while (rows.length < lines) {
    const n = rows.length;
    const suffix = Math.floor(random() * 1_000_000).toString(36).padStart(4, '0');
    const body = n % 97 === 0 ? '# 日本語の編集と音楽の証拠を記録します' : n % 131 === 0 ? '# emoji 🎹🎧 vactr canvas' : `let evidence${n}x${suffix} ${Math.floor(random() * 1000)}`;
    const width = n % 211 === 0 ? 420 + Math.floor(random() * 60) : 24 + Math.floor(random() * 30);
    const comment = body.startsWith('# ') ? body.slice(2) : body;
    rows.push(n % 211 === 0 ? `# ${comment} ${'x'.repeat(Math.max(0, width - comment.length - 3))}` : body.startsWith('# ') ? body : n % 2 === 0 ? `# ${body} ${'x'.repeat(Math.max(0, width - body.length - 3))}` : body);
  }
  let text = rows.join('\n');
  let bytes = encoder.encode(text).length;
  let n = rows.length - 1;
  while (bytes < 1_048_576 * 0.95 && n >= head.length) {
    const old = rows[n]; const replacement = `# payload_${n} ${'p'.repeat(72)}`;
    rows[n] = replacement; bytes += encoder.encode(replacement).length - encoder.encode(old).length;
    n -= 1;
  }
  text = rows.join('\n'); bytes = encoder.encode(text).length;
  if (rows.length !== lines) throw new Error(`expected ${lines} lines, got ${rows.length}`);
  if (bytes < 1_048_576 * 0.95 || bytes > 1_048_576 * 1.05) throw new Error(`UTF-8 size out of range: ${bytes}`);
  return { text, controlText: [...synthDefinition, `stack [${voices}] > d1`].join('\n'), head: head.join('\n'), lines: rows.length, bytes, seed, voices: 64, visualOutputs: ['o0', 'o1', 'o2', 'o3'], longLines: rows.filter((line) => line.length > 400).length };
}
