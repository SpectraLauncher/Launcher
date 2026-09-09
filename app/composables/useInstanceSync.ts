import { invoke } from '@tauri-apps/api/core'
import type {
  JoinAction,
  JoinResolution,
  SyncedPack,
  SyncOption,
  SyncSource,
  SyncState,
} from '~/types/sync'

const emptyState = (): SyncState => ({
  global: {
    game_options: false,
    multiplayer_servers: false,
    command_history: false,
    creative_hotbars: false,
    resource_packs: false,
  },
  excluded: {},
  seeded_from: {},
})

export const useInstanceSync = () => {
  const state = useState<SyncState>('sync-state', emptyState)
  const packs = useState<SyncedPack[]>('sync-packs', () => [])
  const loaded = useState('sync-loaded', () => false)
  const busy = useState<SyncOption | null>('sync-busy', () => null)

  async function load() {
    state.value = await invoke<SyncState>('sync_get_state')
    loaded.value = true
    if (state.value.global.resource_packs) await loadPacks()
  }

  async function ensureLoaded() {
    if (!loaded.value) await load()
  }

  async function loadPacks() {
    packs.value = await invoke<SyncedPack[]>('sync_list_packs')
  }

  function sources() {
    return invoke<SyncSource[]>('sync_sources')
  }

  function joinPreview(instanceId: string, option: SyncOption) {
    return invoke<JoinAction>('sync_join_preview', { instanceId, option })
  }

  async function setGlobal(option: SyncOption, enabled: boolean, baseInstanceId?: string) {
    busy.value = option
    try {
      state.value = await invoke<SyncState>('sync_set_global', {
        option,
        enabled,
        baseInstanceId: baseInstanceId ?? null,
      })
      if (option === 'resource_packs') await loadPacks()
    } finally {
      busy.value = null
    }
  }

  async function setInstance(
    instanceId: string,
    option: SyncOption,
    enabled: boolean,
    resolution?: JoinResolution,
  ) {
    busy.value = option
    try {
      state.value = await invoke<SyncState>('sync_set_instance', {
        instanceId,
        option,
        enabled,
        resolution: resolution ?? null,
      })
    } finally {
      busy.value = null
    }
  }

  function participates(instanceId: string, option: SyncOption): boolean {
    return state.value.global[option] && !state.value.excluded[instanceId]?.[option]
  }

  async function setPackEnabled(packId: string, enabled: boolean) {
    packs.value = await invoke<SyncedPack[]>('sync_set_pack_enabled', { packId, enabled })
  }

  async function removePack(packId: string) {
    packs.value = await invoke<SyncedPack[]>('sync_remove_pack', { packId })
  }

  function openFolder() {
    return invoke<string>('sync_open_folder').then(path => invoke('reveal_in_explorer', { path }))
  }

  return {
    state,
    packs,
    busy,
    load,
    ensureLoaded,
    loadPacks,
    sources,
    joinPreview,
    setGlobal,
    setInstance,
    participates,
    setPackEnabled,
    removePack,
    openFolder,
  }
}
