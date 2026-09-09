import { convertFileSrc, invoke } from '@tauri-apps/api/core'

export function assetUrl(path: string | null | undefined): string {
  return path ? convertFileSrc(path) : ''
}

export function contentIconUrl(item: { icon_url?: string | null, icon_path?: string | null }): string {
  return item.icon_url ?? assetUrl(item.icon_path)
}

let thumbnailQueue: Promise<unknown> = Promise.resolve()

export function thumbnailUrl(path: string, size: number): Promise<string> {
  const next = thumbnailQueue.then(async () => {
    try {
      return assetUrl(await invoke<string>('get_image_thumbnail', { path, size }))
    } catch {
      return assetUrl(path)
    }
  })
  thumbnailQueue = next.catch(() => {})
  return next
}
