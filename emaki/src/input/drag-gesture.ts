const gestures = new WeakSet<Event>();
export function markDragGesture(event: Event): void { gestures.add(event); }
export function isDragGesture(event: Event): boolean { return gestures.has(event); }
