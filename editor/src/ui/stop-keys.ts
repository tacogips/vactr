export type StopShortcut = 'stop-all' | 'cut';

type StopKeyEvent = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

export function isApplePlatform(platform = typeof navigator === 'undefined' ? '' : navigator.platform): boolean {
  return /Mac|iP/.test(platform);
}

function isPeriod(event: StopKeyEvent): boolean {
  return (event.code ?? '') === 'Period' || ((event.code ?? '') === '' && (event.key === '.' || (event.key === '>' && event.shiftKey)));
}

export function stopShortcut(event: StopKeyEvent, apple: boolean): StopShortcut | null {
  if (!isPeriod(event) || event.altKey) return null;
  const mod = apple ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  if (!mod) return null;
  return event.shiftKey ? 'cut' : 'stop-all';
}

export function fallbackStopShortcut(event: StopKeyEvent): StopShortcut | null {
  if (!isPeriod(event) || event.altKey || !(event.metaKey || event.ctrlKey)) return null;
  return event.shiftKey ? 'cut' : 'stop-all';
}
