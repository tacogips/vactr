export function modeColor(img, rect) {
  const counts = new Map();
  for (let y = Math.max(0, rect.top); y < Math.min(img.height, rect.bottom); y++) {
    for (let x = Math.max(0, rect.left); x < Math.min(img.width, rect.right); x++) {
      const i = (y * img.width + x) * 4;
      const key = `${img.data[i]},${img.data[i + 1]},${img.data[i + 2]}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
  }
  let winner = '0,0,0'; let maximum = -1;
  for (const [key, count] of counts) if (count > maximum) { winner = key; maximum = count; }
  return winner.split(',').map(Number);
}

const distance = (img, x, y, color) => {
  const i = (y * img.width + x) * 4;
  return Math.hypot(img.data[i] - color[0], img.data[i + 1] - color[1], img.data[i + 2] - color[2]);
};

export function cellInk(img, rect, backdrop) {
  let brightest = 0;
  for (let y = Math.max(0, rect.top); y < Math.min(img.height, rect.bottom); y++)
    for (let x = Math.max(0, rect.left); x < Math.min(img.width, rect.right); x++) brightest = Math.max(brightest, distance(img, x, y, backdrop));
  if (brightest <= 8) return { ink: 0, inkLeft: null, inkRight: null };
  let ink = 0; let inkLeft = null; let inkRight = null;
  for (let x = Math.max(0, rect.left); x < Math.min(img.width, rect.right); x++) {
    let column = 0;
    for (let y = Math.max(0, rect.top); y < Math.min(img.height, rect.bottom); y++) column += Math.max(0, Math.min(1, distance(img, x, y, backdrop) / brightest));
    ink += column;
    if (column >= 0.25) { if (inkLeft === null) inkLeft = x; inkRight = x; }
  }
  return { ink, inkLeft, inkRight };
}

export function compareLine(canvasCells, referenceCells, chars, tolerances = { minInk: 0.5, extentPx: 1, ratio: [0.8, 1.25] }) {
  const failures = []; const visible = [];
  for (let i = 0; i < chars.length; i++) {
    const canvas = canvasCells[i]; const reference = referenceCells[i];
    if (/\s/u.test(chars[i])) continue;
    visible.push(i);
    if (!reference || !canvas || canvas.ink < reference.ink * tolerances.minInk) failures.push(`P1 char ${i} ${JSON.stringify(chars[i])}: canvas ink ${canvas?.ink ?? 0}, reference ${reference?.ink ?? 0}`);
    if (canvas?.inkLeft !== null && canvas?.inkLeft !== undefined && reference?.inkLeft !== null && reference?.inkLeft !== undefined) {
      const extent = canvas.inkRight - canvas.inkLeft; const expected = reference.inkRight - reference.inkLeft;
      if (Math.abs(extent - expected) > tolerances.extentPx) failures.push(`P3 char ${i} ${JSON.stringify(chars[i])}: extent ${extent}, reference ${expected}`);
    }
  }
  const meanReference = visible.length ? visible.reduce((sum, i) => sum + (referenceCells[i]?.ink ?? 0), 0) / visible.length : 0;
  if (visible.length > 0) for (let i = 0; i < chars.length; i++) if (/\s/u.test(chars[i]) && (canvasCells[i]?.ink ?? 0) > meanReference * 0.15) failures.push(`P2 char ${i} whitespace ink ${canvasCells[i]?.ink ?? 0}, limit ${meanReference * 0.15}`);
  const canvasTotal = visible.reduce((sum, i) => sum + (canvasCells[i]?.ink ?? 0), 0);
  const referenceTotal = visible.reduce((sum, i) => sum + (referenceCells[i]?.ink ?? 0), 0);
  const inkRatio = referenceTotal ? canvasTotal / referenceTotal : visible.length > 0 ? 0 : null;
  if (visible.length > 0 && (inkRatio < tolerances.ratio[0] || inkRatio > tolerances.ratio[1])) failures.push(`P4 line ink ratio ${inkRatio}, expected ${tolerances.ratio.join('..')}`);
  return { failures, inkRatio };
}

export function verticalRuns(img, x, color, tol) {
  const runs = []; let start = null;
  for (let y = 0; y <= img.height; y++) {
    const match = y < img.height && distance(img, x, y, color) <= tol;
    if (match && start === null) start = y;
    if (!match && start !== null) { runs.push([start, y]); start = null; }
  }
  return runs;
}

export function bandRows(img, rect, color, tol) {
  const rows = [];
  for (let y = Math.max(0, rect.top); y < Math.min(img.height, rect.bottom); y++) {
    for (let x = Math.max(0, rect.left); x < Math.min(img.width, rect.right); x++) if (distance(img, x, y, color) <= tol) { rows.push(y); break; }
  }
  return rows;
}

export function columnUniform(img, x, top, bottom, tol) {
  if (top >= bottom) return true;
  const i = (top * img.width + x) * 4; const color = [img.data[i], img.data[i + 1], img.data[i + 2]];
  for (let y = top + 1; y < Math.min(img.height, bottom); y++) if (distance(img, x, y, color) > tol) return false;
  return true;
}
