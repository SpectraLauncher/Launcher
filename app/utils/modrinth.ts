import type { ContentKind, ModrinthProjectType } from '~/types/modrinth'

export function searchProjectType(kind: ContentKind): ModrinthProjectType {
  return kind === 'datapack' ? 'mod' : kind
}

export function baseCategories(kind: ContentKind): string[] {
  return kind === 'datapack' ? ['datapack'] : []
}

export function usesLoaderFilter(kind: ContentKind): boolean {
  return kind === 'mod' || kind === 'modpack'
}

export function loaderFacetFor(kind: ContentKind, loader?: string): string[] {
  if (kind === 'datapack') return ['datapack']
  if (usesLoaderFilter(kind) && loader && loader !== 'vanilla') return [loader]
  return []
}

export function compactNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}
