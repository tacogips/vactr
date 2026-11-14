import type { SpanLanguage } from 'emaki/engine';
import type { SpanProvider } from './syntax';

export function createVactLanguage(initial: SpanProvider): {
  language: SpanLanguage;
  replace(next: SpanProvider): void;
} {
  let provider = initial;
  const language: SpanLanguage = {
    kind: 'spans',
    name: 'vact',
    spans(state, from, to, limit) {
      const result = provider.spans(state, from, to, limit);
      return {
        spans: result.spans.filter((span) => span.kind === 'syntax').map((span) => ({
          from: span.from, to: span.to, tag: span.className ?? '',
        })),
        truncated: result.truncated,
      };
    },
    noteChanges(changes, state) { provider.noteChanges?.(changes, state); },
    dispose() { provider.dispose?.(); },
  };
  return { language, replace(next) { provider = next; } };
}
