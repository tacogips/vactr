/** A shortest edit script, or null when its edit distance exceeds maxCost. */
export function myers<T>(a: readonly T[], b: readonly T[], maxCost: number): ('equal' | 'delete' | 'insert')[] | null {
  const limit = Math.min(a.length + b.length, Math.max(0, Math.floor(maxCost)));
  const offset = limit + 1;
  const v = new Int32Array(2 * limit + 3);
  const trace: Int32Array[] = [];
  for (let d = 0; d <= limit; d++) {
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      const index = offset + k;
      let x = k === -d || (k !== d && v[index - 1]! < v[index + 1]!)
        ? v[index + 1]! : v[index - 1]! + 1;
      let y = x - k;
      while (x < a.length && y < b.length && a[x] === b[y]) { x++; y++; }
      v[index] = x;
      if (x >= a.length && y >= b.length) {
        const result: ('equal' | 'delete' | 'insert')[] = [];
        for (let step = d; step > 0; step--) {
          const previous = trace[step]!;
          const diagonal = x - y;
          const prevK = diagonal === -step || (diagonal !== step && previous[offset + diagonal - 1]! < previous[offset + diagonal + 1]!)
            ? diagonal + 1 : diagonal - 1;
          const prevX = previous[offset + prevK]!;
          const prevY = prevX - prevK;
          while (x > prevX && y > prevY) { result.push('equal'); x--; y--; }
          if (x === prevX) { result.push('insert'); y--; }
          else { result.push('delete'); x--; }
        }
        while (x > 0 && y > 0) { result.push('equal'); x--; y--; }
        return result.reverse();
      }
    }
  }
  return null;
}
