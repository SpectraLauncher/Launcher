export type LoaderType = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge'

export type Loader =
  | { type: 'vanilla' }
  | { type: 'fabric'; version: string }
  | { type: 'quilt'; version: string }
  | { type: 'forge'; version: string }
  | { type: 'neoforge'; version: string }

export interface Instance {
  id: string
  name: string
  mc_version: string
  loader: Loader
  memory_mb?: number | null
  java_args: string[]
  icon?: string | null
  group?: string | null
  created_at: string
  last_played?: string | null
  playtime_seconds: number
  share_origin?: { code: string, revision: number, item_ids: string[] } | null

  override_memory: boolean
  override_window: boolean
  fullscreen: boolean
  width?: number
  height?: number
  override_java_args: boolean
  override_java: boolean
  java_path?: string
  override_env: boolean
  env_vars: EnvVar[]
  override_hooks: boolean
  pre_launch?: string
  wrapper?: string
  post_exit?: string
}

export interface EnvVar {
  key: string
  value: string
}

export type AccountKind = 'microsoft' | 'offline'

export interface Account {
  kind: AccountKind
  uuid: string
  username: string
  xuid: string
  access_token: string
  refresh_token: string
  exp: number
  client_id: string
}

export interface AccountsFile {
  accounts: Account[]
  active_uuid?: string | null
}

export interface Settings {
  default_memory_mb: number
  last_instance_id?: string | null
  theme: string
  default_fullscreen: boolean
  default_width?: number
  default_height?: number
  default_java_path?: string
  default_java_args: string[]
  default_env_vars: EnvVar[]
  default_pre_launch?: string
  default_wrapper?: string
  default_post_exit?: string
  track_playtime: boolean
  share_activity: boolean
  discord_rpc: boolean
  crash_reports: boolean
  anonymous_stats: boolean
  snapshot_before_updates: boolean
  snapshot_keep: number
}

export interface SavedSkin {
  id: string
  name: string
  model: 'classic' | 'slim'
  active: boolean
  created_at: string
}

export interface PlayerSkin {
  skin: string
  slim: boolean
}

export interface LauncherPaths {
  data_root: string
  instances: string
  runtimes: string
  skins: string
  cache: string
  logs: string
}

export interface MultiProgress { instance_id: string; current: number; total: number }
export interface FileProgress { instance_id: string; path: string; current: number; total: number }
export interface ConsoleLine { instance_id: string; line: string }
export interface ModpackProgress { instance_id: string; name: string; current: number; total: number }
export interface ExitInfo { instance_id: string; code: number | null }
export interface CrashInfo {
  instance_id: string
  code: number | null
  crash_report_rel: string | null
}

export type QuickPlay =
  | { kind: 'Singleplayer'; world: string }
  | { kind: 'Multiplayer'; host: string; port?: number }

export interface PingResult {
  latency_ms: number
  version: string
  protocol: number
  online: number
  max: number
  motd: string
  favicon: string | null
}

export interface ScreenshotInfo { name: string; path: string; modified: number }
export interface WorldInfo {
  folder: string
  name: string
  icon_path: string | null
  last_played: number | null
  version: string | null
  game_mode: string | null
}
export interface PackInfo {
  name: string
  filename: string
  description: string | null
  pack_format: number | null
  icon: string | null
  is_zip: boolean
  enabled: boolean
}
export interface ShaderInfo { name: string; filename: string; is_zip: boolean; enabled: boolean }
export interface ServerInfo { name: string; ip: string; icon: string | null; hidden: boolean }
export interface DirChild {
  name: string
  is_dir: boolean
  size: number
}

export interface ExternalInstance {
  launcher: 'prism' | 'curseforge' | 'modrinth'
  name: string
  path: string
  game_dir: string
  mc_version: string | null
  loader: string | null
  loader_version: string | null
}

export interface BlockedMod {
  name: string
  filename: string
  project_id: string
  file_id: string
  url: string
  fingerprint: number
}

export interface LogFile {
  name: string
  kind: 'latest' | 'log' | 'archived' | 'crash'
  rel: string
  modified: number
  size: number
}

export interface UnresolvedFile {
  path: string
  size: number
}

export interface SharePreview {
  modrinth: number
  curseforge: number
  unresolved: UnresolvedFile[]
  unresolved_bytes: number
}

export interface ShareResult {
  code: string
  url: string
  expires: number
}

export interface ShareImportResult {
  instance: Instance
  installed: number
  failed: string[]
  needs_curseforge: number
}

export interface ModEntry {
  filename: string
  enabled: boolean
  name: string | null
  version: string | null
  version_id: string | null
  icon_url: string | null
  project_id: string | null
  provider: 'local' | 'modrinth' | 'curseforge'
  modified: number
}
