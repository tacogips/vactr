export type StopShortcut = 'stop-all' | 'cut';

type StopKeyEvent = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

export function isApplePlatform(platform = typeof navigator === 'undefined' ? '' : navigator.platform): boolean {
  return /Mac|iP/.test(platform);
}

export function stopShortcut(event: StopKeyEvent, apple: boolean): StopShortcut | null {
  const period = event.code === 'Period' || (event.code === '' && (event.key === '.' || (event.key === '>' && event.shiftKey)));
  if (!period || event.altKey) return null;
  const mod = apple ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
  if (!mod) return null;
  return event.shiftKey ? 'cut' : 'stop-all';
}
