import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { LoaderType, Instance } from '~/types/launcher'
import type { ContentKind } from '~/types/modrinth'

export interface ContentWindowConfig {
  kind: ContentKind
  mode: 'install' | 'createModpack'
  instanceId?: string
  gameVersion?: string
  loader?: LoaderType
  query?: string
}

export interface ContentInstalled {
  instanceId?: string
  instance?: Instance
}

export const useContentWindow = () => {
  const open = (config: ContentWindowConfig) => invoke('open_content_window', { config })

  const close = () => invoke('close_content_window').catch(() => {})

  const announce = (payload: ContentInstalled) =>
    invoke('content_installed', { payload }).catch(() => {})

  const onInstalled = (handler: (payload: ContentInstalled) => void): Promise<UnlistenFn> =>
    listen<ContentInstalled>('content://installed', e => handler(e.payload))

  return { open, close, announce, onInstalled }
}
