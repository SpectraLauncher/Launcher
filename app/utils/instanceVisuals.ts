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

const NEUTRAL_BADGE = 'bg-elevated text-toned ring-1 ring-inset ring-(--ui-border-accented)'

export const LOADER_BADGE_CLASS: Record<LoaderType, string> = {
  vanilla: NEUTRAL_BADGE,
  fabric: NEUTRAL_BADGE,
  quilt: NEUTRAL_BADGE,
  forge: NEUTRAL_BADGE,
  neoforge: NEUTRAL_BADGE,
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
  return `linear-gradient(160deg, hsl(${h} 42% 44%), hsl(${(h + 16) % 360} 40% 33%))`
}

export function instanceInitial(instance: Pick<Instance, 'name'>): string {
  return instance.name.trim().charAt(0).toUpperCase() || '?'
}

export function instanceSubtitle(instance: Pick<Instance, 'mc_version' | 'loader'>): string {
  return `${instance.mc_version} · ${loaderLabel(instance.loader.type)}`
}
