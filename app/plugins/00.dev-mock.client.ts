// Dev-only: lets `pnpm dev` render in a plain browser by answering Tauri IPC
// with sample data. Inert inside the real Tauri window and in builds.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

export default defineNuxtPlugin(() => {
  if (!import.meta.dev || '__TAURI_INTERNALS__' in window) return

  ;(window as any).__TAURI_OS_PLUGIN_INTERNALS__ = { platform: 'windows', os_type: 'windows', family: 'windows', version: '10', arch: 'x86_64', eol: '\r\n', exe_extension: 'exe' }
  mockWindows('main')
  ;(window as any).__TAURI_INTERNALS__.convertFileSrc = (path: string) => path
  localStorage.setItem('spectra-onboarded', '1')
  // `?shot` hides the Nuxt DevTools pill so screenshots show only the launcher.
  if (new URLSearchParams(location.search).has('shot')) {
    const style = document.createElement('style')
    style.textContent = '#nuxt-devtools-container, nuxt-devtools-inspect-panel { display: none !important; }'
    document.head.appendChild(style)
  }
  const settings = {
    default_memory_mb: 6144, theme: 'dark', default_fullscreen: false, default_java_args: [], default_env_vars: [],
    track_playtime: true, share_activity: false, discord_rpc: true, crash_reports: true, anonymous_stats: false,
    snapshot_before_updates: true, snapshot_keep: 5, addon_dev_mode: false, open_console_on_launch: false,
  }

  const day = 86_400_000
  const ago = (d: number) => new Date(Date.now() - d * day).toISOString()
  const inst = (id: string, name: string, mc: string, loader: string, played: number | null, hours: number, group: string | null = null) => ({
    id, name, mc_version: mc, loader: { type: loader, version: loader === 'neoforge' ? '21.1.172' : '0.16.14' }, java_args: [], icon: id in ICONS, group,
    created_at: ago(90), last_played: played === null ? null : ago(played), playtime_seconds: hours * 3600,
    override_memory: false, override_window: false, fullscreen: false, override_java_args: false, override_java: false,
    override_env: false, env_vars: [], override_hooks: false,
  })
  const ICONS: Record<string, string> = {
    fo: 'https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp',
    cobblemon: 'https://cdn.modrinth.com/data/5FFgwNNP/e7f9ee2e9d361623847853fe2ddce42f519ee64f.png',
    bmc: 'https://cdn.modrinth.com/data/shFhR8Vx/a19c2bcb51d38f32f138d3607e91cb2b7b8e387f_96.webp',
    prominence: 'https://cdn.modrinth.com/data/EGs3lC8D/a0d0d63375454061b73003d0a47dd92503604a64_96.webp',
  }
  const instances = [
    inst('fo', 'Fabulously Optimized', '1.21.4', 'fabric', 0.05, 41),
    inst('cobblemon', 'Cobblemon Official Modpack', '1.21.1', 'fabric', 1, 63),
    { ...inst('sprocket', 'SprocketSMP', '1.21.1', 'neoforge', 2, 52), share_origin: { code: 'K7PX2Q', revision: 3, item_ids: ['p0', 'p1', 'p3'] } },
    inst('create', 'Create with friends', '1.21.1', 'neoforge', 4, 27),
    inst('bmc', 'Better MC [FABRIC] - BMC2', '1.20.1', 'fabric', 9, 19),
    inst('prominence', 'Prominence II: Hasturian Era', '1.20.1', 'fabric', 21, 14),
    inst('vanilla', 'Vanilla 1.21.4', '1.21.4', 'vanilla', 30, 8),
  ]
  if (!localStorage.getItem('spectra-library-layout')) {
    localStorage.setItem('spectra-library-layout', JSON.stringify([
      { id: 'fav', name: 'Favourites', collapsed: false, ids: ['fo', 'cobblemon', 'sprocket'] },
      { id: '__ungrouped', name: null, collapsed: false, ids: ['create', 'bmc', 'prominence', 'vanilla'] },
    ]))
  }
  const MOD_SLUGS = ['sodium', 'iris', 'lithium', 'create', 'jei', 'jade', 'xaeros-minimap', 'appleskin', 'mouse-tweaks', 'fabric-api', 'cloth-config', 'modmenu', 'farmers-delight', 'supplementaries', 'waystones', 'sophisticated-backpacks', 'ferrite-core', 'entityculling', 'immediatelyfast', 'continuity', 'distanthorizons', 'chunky', 'simple-voice-chat', 'emi']
  const VERSIONS = ['0.6.13', '1.8.8', '0.14.7', '6.0.6', '19.21.0', '15.10.2', '25.2.10', '3.0.6', '2.26', '0.116.3', '15.0.140', '11.0.3', '1.2.9', '3.1.31', '21.1.13', '3.23.6', '7.0.2', '1.7.4', '1.3.4', '3.0.0', '2.3.2', '1.4.27', '2.5.30', '1.1.22']
  const api = (path: string) => fetch(`https://api.modrinth.com/v2${path}`).then(r => r.json())
  let modsPromise: Promise<any[]> | null = null
  const loadMods = () => (modsPromise ??= api(`/projects?ids=${encodeURIComponent(JSON.stringify(MOD_SLUGS))}`)
    .then((projects: any[]) => MOD_SLUGS.map((slug, i) => {
      const p = projects.find(x => x.slug === slug)
      const name = (p?.title ?? slug).replace(/ 🔍$/, '')
      return {
        filename: `${slug}-${VERSIONS[i]}.jar`, enabled: ![5, 13].includes(i), name,
        version: VERSIONS[i], version_id: `v${i}`, icon_url: p?.icon_url ?? null, icon_path: null,
        project_id: p?.id ?? `p${i}`, provider: i === 22 ? 'local' : 'modrinth',
        modified: Date.now() - i * day * 1.7, cloud: false,
      }
    })))
  const searchHits = (params: any) => {
    const facets = [[`project_type:${params?.project_type ?? 'mod'}`], ...(params?.game_versions?.length ? [params.game_versions.map((v: string) => `versions:${v}`)] : []), ...(params?.loaders?.length ? [params.loaders.map((l: string) => `categories:${l}`)] : [])]
    const q = new URLSearchParams({ query: params?.query ?? '', limit: String(params?.limit ?? 20), offset: String(params?.offset ?? 0), index: params?.index ?? 'relevance', facets: JSON.stringify(facets) })
    return api(`/search?${q}`)
  }
  const skinData = (url: string) => fetch(url).then(r => r.blob()).then(b => new Promise<string>((res) => {
    const fr = new FileReader()
    fr.onload = () => res(fr.result as string)
    fr.readAsDataURL(b)
  }))
  const STEVE = 'https://textures.minecraft.net/texture/31f477eb1a7beee631c2ca64d06f8f68fa93a3386d04452ab27f43acdf1b60cb'
  const ALEX = 'https://textures.minecraft.net/texture/46acd06e8483b176e8ea39fc12fe105eb3a2a4970f5100057e9d84d4b60bdfa7'
  const worlds = (id: string) => ({
    fo: [['New World', 0.05, 'survival'], ['Creative flat', 12, 'creative']],
    cobblemon: [['Pokemon journey', 1, 'survival'], ['Gym battles', 6, 'adventure']],
    sprocket: [['SprocketSMP season 3', 2, 'survival']],
    create: [['Factory island', 4, 'survival'], ['Train network test', 8, 'creative'], ['Brass age', 19, 'survival']],
    bmc: [['Hardcore run', 9, 'hardcore']],
  } as Record<string, [string, number, string][]>)[id]?.map(([name, played, mode]) => ({
    folder: name.toLowerCase().replace(/\W+/g, '-'), name, icon_path: null, last_played: Date.now() - played * day,
    version: instances.find(i => i.id === id)?.mc_version ?? null, game_mode: mode,
  })) ?? []

  let presence = 'visible'
  mockIPC((cmd, args: any) => {
    switch (cmd) {
      case 'list_instances': return instances
      case 'list_accounts': return { accounts: [{ kind: 'microsoft', uuid: 'u1', username: 'makotopd', xuid: '', access_token: '', refresh_token: '', exp: 0, client_id: '' }], active_uuid: 'u1' }
      case 'list_content': return args?.kind === 'mod' ? loadMods() : []
      case 'list_mods': return loadMods()
      case 'get_instance_icon_path': return ICONS[args?.id] ?? null
      case 'list_worlds': return worlds(args?.id)
      case 'get_player_skin': return skinData(STEVE).then(skin => ({ skin, slim: false }))
      case 'list_default_skins': return [{ name: 'Steve', model: 'classic', path: null, url: STEVE }, { name: 'Alex', model: 'slim', path: null, url: ALEX }]
      case 'fetch_skin_data_url': return skinData(args?.url)
      case 'list_skins': return [
        { id: 's1', name: 'Explorer', model: 'classic', active: true, created_at: ago(20) },
        { id: 's2', name: 'Builder', model: 'slim', active: false, created_at: ago(5) },
      ]
      case 'get_skin_data_url': return skinData(args?.id === 's2' ? ALEX : STEVE)
      case 'get_instance': return instances.find(i => i.id === args?.id || i.id === args?.instanceId) ?? instances[0]
      case 'is_instance_running': return false
      case 'plugin:event|listen': return Math.floor(Math.random() * 1e6)
      case 'plugin:store|load': return 1
      case 'plugin:store|get': return [null, false]
      case 'plugin:app|version': return '1.1.0'
      case 'get_settings': return settings
      case 'spectra_session': return { id: 'me', name: 'Makoto', username: 'makotopd', email: 'makoto@example.com', image: null, emailVerified: true, mcUsername: 'makotopd' }
      case 'spectra_api':
        if (args?.path === '/api/friends') return {
          presence,
          incoming: [{ id: 7, created: Date.now(), user: { id: 'u7', name: 'Kuba', username: 'kubaa', image: null, status: 'online' } }],
          outgoing: [],
          friends: [
            { id: 'u1', name: 'Ola', username: 'ola_mc', image: null, mcUsername: 'OlaBuilds', friendshipId: 1, status: 'in_game' },
            { id: 'u2', name: 'Tomek', username: 'tomek', image: null, mcUsername: 'Tomeczek', friendshipId: 2, status: 'online' },
            { id: 'u3', name: 'Zuza', username: 'zuza', image: null, friendshipId: 3, status: 'dnd' },
            { id: 'u4', name: 'Bartek', username: 'bartek', image: null, friendshipId: 4, status: 'offline' },
          ],
        }
        if (args?.path === '/api/presence') { presence = args.body.mode; return null }
        if (args?.path?.startsWith('/api/notifications') && args.method === 'GET') return { unread: 0, notifications: [] }
        if (args?.path === '/api/shares') return { shares: [{
          code: 'K7PX2Q', instance_id: null, name: 'SprocketSMP', revision: 4, downloads: 5, expires: Date.now() + 5 * day,
          canExtend: false, role: 'collaborator', owner: { id: 'u1', name: 'Ola', username: 'ola_mc', image: null },
          recipients: [
            { user: { id: 'me', name: 'Makoto', username: 'makotopd', image: null }, importedRevision: 3, outdated: true, canPush: true },
            { user: { id: 'u2', name: 'Tomek', username: 'tomek', image: null }, importedRevision: 4, outdated: false, canPush: false },
          ],
        }] }
        return []
      case 'modrinth_search':
      case 'curseforge_search': return searchHits(args?.params)
      case 'modrinth_versions':
      case 'curseforge_versions': {
        const q = new URLSearchParams()
        if (args?.loaders?.length) q.set('loaders', JSON.stringify(args.loaders))
        if (args?.gameVersions?.length) q.set('game_versions', JSON.stringify(args.gameVersions))
        return api(`/project/${args?.projectId ?? args?.project_id}/version?${q}`).catch(() => [])
      }
      case 'modrinth_categories':
      case 'curseforge_categories': return api('/tag/category').then((c: any[]) => c.filter(x => x.project_type === (args?.projectType ?? 'mod'))).catch(() => [])
      case 'modrinth_project': return api(`/project/${args?.projectId ?? args?.id}`).catch(() => null)
      case 'share_preview': return { modrinth: 18, curseforge: 6, unresolved: [], unresolved_bytes: 0 }
      case 'share_instance': throw { code: 'share_stale', message: 'revision 4 was pushed since you last updated — get it first' }
      case 'share_changes': return { revision: 4, changes: [
        { project_id: 'p3', name: 'Create', kind: 'mod', ours: '6.0.4', theirs: '6.0.6', by: 'them' },
        { project_id: 'p9', name: 'Distant Horizons', kind: 'mod', ours: null, theirs: '2.3.0', by: 'them' },
        { project_id: 'p1', name: 'Iris Shaders', kind: 'mod', ours: '1.8.1', theirs: null, by: 'them' },
        { project_id: 'p12', name: 'Supplementaries', kind: 'mod', ours: '3.1.2', theirs: null, by: 'you' },
        { project_id: 'p0', name: 'Sodium', kind: 'mod', ours: '0.6.3', theirs: '0.6.5', by: 'both' },
      ] }
      case 'content_window_config': return { kind: 'mod', mode: 'install', instanceId: 'sprocket', gameVersion: '1.21.1', loader: 'neoforge' }
      case 'get_system_memory': return { total_mb: 32768, available_mb: 18000 }
    }
    if (cmd.startsWith('cloud_sync_')) return { accountId: '', enabled: false, revision: 0, updated: null, status: 'off', changed: false }
    if (/^(list|get_all|scan|find)_|s$/.test(cmd)) return []
    if (/^(is|has|can)_/.test(cmd)) return false
    return null
  }, { shouldMockEvents: true })
})
