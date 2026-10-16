import type { ExampleEntry, ExampleSource } from '../documents/types';
import { CURATED, DEFAULT_EXAMPLE_ID, EXCLUDED, TRACK_CATEGORY_PREFIX } from './manifest';

const SOURCES = import.meta.glob<string>('../../../examples/**/*.vact', {
  query: '?raw',
  import: 'default',
});
const CATALOG = import.meta.glob<string>('../../../examples/tracks/catalog.json', {
  query: '?raw',
  import: 'default',
  eager: true,
});

const ROOT = '../../../examples/';
const idOf = (path: string): string => path.slice(ROOT.length, -'.vact'.length);
const excluded = new Set(EXCLUDED.map(({ path }) => path.slice(0, -'.vact'.length)));
const excludedPathSet = new Set(excluded);
const loaders = new Map(Object.entries(SOURCES).map(([path, load]) => [idOf(path), load]));

interface TrackRow {
  g: string;
  slug: string;
  title: string;
  bpm: number;
  desc: string;
}
const catalogText = Object.values(CATALOG)[0];
const catalog: TrackRow[] = catalogText ? JSON.parse(catalogText) as TrackRow[] : [];

const tracks: ExampleEntry[] = catalog
  .filter((row) => !excluded.has(`tracks/${row.g}/${row.slug}`))
  .map((row) => ({
    id: `tracks/${row.g}/${row.slug}`,
    title: row.title,
    description: `${row.desc} ${row.bpm} BPM.`.slice(0, 140),
    category: `${TRACK_CATEGORY_PREFIX}${row.g}`,
    requires: [],
  }));

const entries: readonly ExampleEntry[] = Object.freeze([
  ...CURATED.map((entry) => ({ ...entry, requires: entry.requires ?? [] })),
  ...tracks,
]);
const byId = new Map(entries.map((entry) => [entry.id, entry]));

export const REGISTRY_FILES: readonly string[] = Object.freeze([...loaders.keys()]);

export function listExamples(): readonly ExampleEntry[] {
  return entries;
}

export function findExample(id: string): ExampleEntry | undefined {
  return byId.get(id);
}

export async function loadExample(id: string): Promise<string> {
  if (!byId.has(id)) throw new Error(`unknown example ${id}`);
  const loader = loaders.get(id);
  if (!loader) throw new Error(`missing example source ${id}`);
  const module = await loader();
  return module;
}

export const exampleSource: ExampleSource = Object.freeze({
  defaultId: DEFAULT_EXAMPLE_ID,
  list: listExamples,
  find: findExample,
  load: loadExample,
});

export function excludedIds(): readonly string[] {
  return [...excludedPathSet];
}
