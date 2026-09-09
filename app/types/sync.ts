export type SyncOption =
  | 'game_options'
  | 'multiplayer_servers'
  | 'command_history'
  | 'creative_hotbars'
  | 'resource_packs'

export const SYNC_OPTIONS: SyncOption[] = [
  'game_options',
  'multiplayer_servers',
  'resource_packs',
  'command_history',
  'creative_hotbars',
]

export type SyncFlags = Record<SyncOption, boolean>

export interface SyncState {
  global: SyncFlags
  excluded: Record<string, SyncFlags>
  seeded_from: Record<string, string>
}

export type JoinAction = 'seed_shared' | 'attach' | 'merge' | 'requires_resolution'
export type JoinResolution = 'use_synced' | 'use_instance'

export interface SyncSource {
  id: string
  name: string
  mc_version: string
  has_icon: boolean
}

export interface SyncedPack {
  id: string
  filename: string
  pack_format: number | null
  enabled: boolean
}
