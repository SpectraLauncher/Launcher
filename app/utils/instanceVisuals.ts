import type { Instance, LoaderType } from '~/types/launcher'

export const LOADER_LABELS: Record<LoaderType, string> = {
  vanilla: 'Vanilla',
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
}

export function loaderLabel(type: LoaderType): string {
  return LOADER_LABELS[type] ?? type
}

export const LOADER_BADGE_CLASS: Record<LoaderType, string> = {
  vanilla: 'bg-green-500/10 text-green-400 ring-1 ring-inset ring-green-500/25',
  fabric: 'bg-blue-500/10 text-blue-400 ring-1 ring-inset ring-blue-500/25',
  quilt: 'bg-purple-500/10 text-purple-400 ring-1 ring-inset ring-purple-500/25',
  forge: 'bg-red-500/10 text-red-400 ring-1 ring-inset ring-red-500/25',
  neoforge: 'bg-orange-500/10 text-orange-400 ring-1 ring-inset ring-orange-500/25',
}

export function loaderBadgeClass(type: LoaderType): string {
  return LOADER_BADGE_CLASS[type] ?? LOADER_BADGE_CLASS.vanilla
}

function hueFromString(s: string): number {
  let h = 0
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360
  return h
}

export function instanceIconBg(instance: Pick<Instance, 'id' | 'name'>): string {
  const h = hueFromString(instance.id || instance.name)
  return `linear-gradient(135deg, hsl(${h} 65% 52%), hsl(${(h + 28) % 360} 60% 42%))`
}

export function instanceInitial(instance: Pick<Instance, 'name'>): string {
  return instance.name.trim().charAt(0).toUpperCase() || '?'
}

export function instanceSubtitle(instance: Pick<Instance, 'mc_version' | 'loader'>): string {
  return `${instance.mc_version} · ${loaderLabel(instance.loader.type)}`
}
