export const GLIDE_KEY = 'vactr.momentary.glideMs', GLIDE_DEFAULT_MS = 1000, GLIDE_MAX_MS = 10000, GLIDE_STEP_MS = 50;
function storageOf(storage?: Storage): Storage | undefined { try { return storage ?? globalThis.localStorage; } catch { return undefined; } }
export function writeGlide(ms: number, storage?: Storage): number {
  const value = Math.round(Math.max(0, Math.min(GLIDE_MAX_MS, Number.isFinite(ms) ? ms : GLIDE_DEFAULT_MS)) / GLIDE_STEP_MS) * GLIDE_STEP_MS;
  try { storageOf(storage)?.setItem(GLIDE_KEY, String(value)); } catch { /* preference storage may be unavailable */ }
  return value;
}
export function readGlide(storage?: Storage): number {
  try { const raw = storageOf(storage)?.getItem(GLIDE_KEY); return raw === null || raw === undefined ? GLIDE_DEFAULT_MS : writeGlide(Number(raw), storage); }
  catch { return GLIDE_DEFAULT_MS; }
}
export function mountGlideSetting(parent: HTMLElement, onChange: (ms: number) => void): { dispose(): void } {
  const doc = parent.ownerDocument, row = doc.createElement('label'); row.className = 'vact-glide-setting';
  const title = doc.createElement('span'); title.textContent = 'momentary glide (ms)';
  const number = doc.createElement('input'); number.type = 'number'; number.min = '0'; number.max = String(GLIDE_MAX_MS); number.step = String(GLIDE_STEP_MS); number.value = String(readGlide());
  const range = doc.createElement('input'); range.type = 'range'; range.min = '0'; range.max = String(GLIDE_MAX_MS); range.step = String(GLIDE_STEP_MS); range.value = number.value;
  const change = (input: HTMLInputElement): void => { const value = writeGlide(Number(input.value)); number.value = range.value = String(value); onChange(value); };
  const onNumber = (): void => change(number), onRange = (): void => change(range);
  number.addEventListener('change', onNumber); range.addEventListener('input', onRange);
  row.append(title, number, range); parent.prepend(row);
  return { dispose() { number.removeEventListener('change', onNumber); range.removeEventListener('input', onRange); row.remove(); } };
}
