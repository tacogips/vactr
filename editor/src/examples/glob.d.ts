interface ImportMeta {
  glob<T = unknown>(
    patterns: string | string[],
    options: { query?: string; import?: string; eager: true },
  ): Record<string, T>;
  glob<T = unknown>(
    patterns: string | string[],
    options?: { query?: string; import?: string; eager?: boolean },
  ): Record<string, () => Promise<T>>;
}
