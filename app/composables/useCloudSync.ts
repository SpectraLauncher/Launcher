import { invoke } from '@tauri-apps/api/core'

export interface CloudSyncView {
  accountId: string
  enabled: boolean
  revision: number
  updated: number | null
  status: 'off' | 'idle' | 'synced' | 'conflict' | 'deferred' | 'signed_out'
  changed: boolean
}

const initial: CloudSyncView = { accountId: '', enabled: false, revision: 0, updated: null, status: 'off', changed: false }

export function useCloudSync() {
  const view = useState<CloudSyncView>('cloud-sync-view', () => ({ ...initial }))
  const busy = useState('cloud-sync-busy', () => false)
  const error = useState<string | null>('cloud-sync-error', () => null)
  const instances = useInstancesStore()

  async function call(command: string, args?: Record<string, unknown>) {
    if (busy.value) return view.value
    busy.value = true
    error.value = null
    try {
      view.value = await invoke<CloudSyncView>(command, args)
      if (view.value.changed) await instances.load()
      return view.value
    } catch (e) {
      error.value = errorText(e)
      throw e
    } finally {
      busy.value = false
    }
  }

  function load() { return call('cloud_sync_state') }
  function tick() { return call('cloud_sync_tick') }
  async function setEnabled(enabled: boolean) {
    await call('cloud_sync_set_enabled', { enabled })
    if (enabled) await tick()
  }
  function resolve(choice: 'cloud' | 'computer') { return call('cloud_sync_resolve', { choice }) }

  async function openBackups() {
    const path = await invoke<string>('cloud_sync_backup_folder')
    await invoke('reveal_in_explorer', { path })
  }

  return { view, busy, error, load, tick, setEnabled, resolve, openBackups }
}
