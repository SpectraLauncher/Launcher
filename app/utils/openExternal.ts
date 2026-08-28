import { invoke } from '@tauri-apps/api/core'

export function openExternal(url: string): Promise<void> {
  return invoke('open_external', { url })
}
