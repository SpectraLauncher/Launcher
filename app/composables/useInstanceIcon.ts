import { invoke, convertFileSrc } from '@tauri-apps/api/core'

const cache = new Map<string, string>()

export async function resolveInstanceIcon(id: string, hasIcon: boolean): Promise<string | null> {
  if (!hasIcon) return null
  const cached = cache.get(id)
  if (cached) return cached
  const path = await invoke<string | null>('get_instance_icon_path', { id })
  if (!path) return null
  const url = `${convertFileSrc(path)}?t=${Date.now()}`
  cache.set(id, url)
  return url
}

export function invalidateInstanceIcon(id: string) {
  cache.delete(id)
}
