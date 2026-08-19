<template>
  <div class="space-y-4">
    <!-- toolbar -->
    <div class="flex items-center justify-between">
      <p class="text-xs text-muted">
        {{ loading ? $t('common.loading') : $t('content.count', { n: count }) }}
      </p>
      <div class="flex items-center gap-1.5">
        <UButton
          v-if="tab === 'servers'"
          icon="i-lucide-plus"
          size="xs"
          :label="$t('content.addServer')"
          @click="addServerOpen = true"
        />
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :loading="loading"
          :label="$t('content.refresh')"
          @click="load"
        />
      </div>
    </div>

    <p v-if="error" class="text-sm text-error">{{ error }}</p>

    <!-- empty -->
    <div v-else-if="!loading && count === 0" class="flex flex-col items-center justify-center gap-3 py-16 text-center">
      <UIcon :name="emptyIcon" class="size-10 text-neutral-600" />
      <p class="text-sm text-muted">{{ $t('content.empty') }}</p>
    </div>

    <!-- loading skeletons (grid for screenshots, rows for everything else) -->
    <div
      v-else-if="loading"
      :class="tab === 'screenshots' ? 'grid gap-3' : 'space-y-2'"
      :style="tab === 'screenshots' ? 'grid-template-columns:repeat(auto-fill,minmax(220px,1fr))' : undefined"
    >
      <template v-if="tab === 'screenshots'">
        <div v-for="n in 6" :key="`ss-sk-${n}`" class="overflow-hidden rounded-xl border border-default bg-white/3">
          <div class="aspect-video w-full animate-pulse bg-white/5" />
          <div class="px-2.5 py-1.5"><div class="h-3 w-2/3 animate-pulse rounded bg-white/5" /></div>
        </div>
      </template>
      <template v-else>
        <div v-for="n in 6" :key="`row-sk-${n}`" class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3">
          <div class="size-11 shrink-0 animate-pulse rounded-lg bg-white/5" />
          <div class="min-w-0 flex-1 space-y-2">
            <div class="h-3.5 w-1/3 animate-pulse rounded bg-white/5" />
            <div class="h-2.5 w-1/2 animate-pulse rounded bg-white/5" />
          </div>
        </div>
      </template>
    </div>

    <!-- screenshots: image grid -->
    <div
      v-else-if="tab === 'screenshots'"
      class="grid gap-3"
      style="grid-template-columns:repeat(auto-fill,minmax(220px,1fr))"
    >
      <button
        v-for="(s, i) in screenshots"
        :key="s.path"
        type="button"
        class="group overflow-hidden rounded-xl border border-default bg-white/3 text-left transition hover:border-primary-500/40"
        @click="lightboxIndex = i"
      >
        <img :src="assetUrl(s.path)" loading="lazy" class="aspect-video w-full object-cover transition group-hover:opacity-90" :alt="s.name" >
        <div class="truncate px-2.5 py-1.5 text-[11px] text-neutral-400" :title="s.name">{{ s.name }}</div>
      </button>
    </div>

    <!-- worlds -->
    <div v-else-if="tab === 'worlds'" class="space-y-2">
      <div
        v-for="w in worlds"
        :key="w.folder"
        class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3"
      >
        <img
          v-if="w.icon_path"
          :src="assetUrl(w.icon_path)"
          class="size-11 shrink-0 rounded-lg object-cover"
          :alt="w.name"
        />
        <div v-else class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-white/5">
          <UIcon name="i-lucide-globe" class="size-5 text-neutral-500" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="truncate font-medium">{{ w.name }}</div>
          <div class="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-neutral-400">
            <span v-if="w.version" class="font-mono">{{ w.version }}</span>
            <span v-if="w.game_mode">· {{ $t(`content.gameMode.${w.game_mode}`) }}</span>
            <span v-if="w.last_played">· {{ formatDate(w.last_played) }}</span>
          </div>
        </div>
        <!-- Quick Play button (MC 1.20+) -->
        <UButton
          v-if="instanceSupportsQuickPlay"
          icon="i-lucide-play"
          size="xs"
          color="neutral"
          variant="ghost"
          :disabled="isRunning"
          :title="$t('quickPlay.playWorld')"
          square
          @click="quickPlayWorld(w.folder)"
        />
        <UButton
          icon="i-lucide-archive"
          size="xs"
          color="neutral"
          variant="ghost"
          :loading="busyWorld === w.folder"
          :title="$t('content.backup')"
          square
          @click="backupWorld(w)"
        />
        <UButton
          icon="i-lucide-trash-2"
          size="xs"
          color="error"
          variant="ghost"
          :title="$t('common.remove')"
          square
          @click="deleteWorld(w)"
        />
      </div>
    </div>

    <!-- servers -->
    <div v-else-if="tab === 'servers'" class="space-y-2">
      <div
        v-for="(s, i) in servers"
        :key="`${s.ip}-${i}`"
        class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3"
        :class="{ 'opacity-55': s.hidden }"
      >
        <!-- Favicon: from ping result > server NBT > placeholder -->
        <div class="relative size-11 shrink-0">
          <img
            v-if="pingFor(s.ip)?.favicon || s.icon"
            :src="pingFor(s.ip)?.favicon ?? s.icon ?? ''"
            class="size-11 rounded-lg object-cover [image-rendering:pixelated]"
            :alt="s.name"
          />
          <div v-else class="flex size-11 items-center justify-center rounded-lg bg-white/5">
            <UIcon name="i-lucide-server" class="size-5 text-neutral-500" />
          </div>
        </div>

        <!-- Info -->
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span class="truncate font-medium">{{ s.name || s.ip }}</span>
            <UBadge v-if="s.hidden" color="neutral" variant="subtle" size="xs" :label="$t('content.hidden')" />
          </div>
          <!-- MOTD from ping -->
          <div v-if="pingFor(s.ip)?.motd" class="mt-0.5 truncate text-[11px] text-neutral-400" :title="pingFor(s.ip)!.motd">
            {{ pingFor(s.ip)!.motd }}
          </div>
          <div v-else class="truncate font-mono text-[11px] text-neutral-500">{{ s.ip }}</div>
        </div>

        <!-- Ping status (right side) -->
        <div class="flex shrink-0 items-center gap-2">
          <!-- Loading -->
          <UIcon v-if="pingStatus(s.ip) === 'loading'" name="i-lucide-loader-circle" class="size-3.5 animate-spin text-neutral-500" />
          <!-- Online -->
          <template v-else-if="pingFor(s.ip)">
            <span :class="latencyClass(pingFor(s.ip)!.latency_ms)" class="font-mono text-[11px]">{{ pingFor(s.ip)!.latency_ms }}ms</span>
            <span class="text-[11px] text-neutral-500">{{ pingFor(s.ip)!.online }}/{{ pingFor(s.ip)!.max }}</span>
          </template>
          <!-- Offline -->
          <span v-else-if="pingStatus(s.ip) === 'offline'" class="text-[11px] text-neutral-600">{{ $t('server.offline') }}</span>
        </div>

        <!-- Actions -->
        <div class="flex shrink-0 items-center gap-1">
          <UButton
            v-if="instanceSupportsQuickPlay"
            icon="i-lucide-play"
            size="xs"
            color="primary"
            variant="ghost"
            :disabled="isRunning"
            :title="$t('quickPlay.connectServer')"
            square
            @click="quickPlayServer(s.ip)"
          />
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            size="xs"
            :title="$t('common.remove')"
            @click="removeServer(i)"
          />
        </div>
      </div>
    </div>

    <!-- add server modal -->
    <UModal v-model:open="addServerOpen" :title="$t('content.addServer')">
      <template #body>
        <div class="space-y-3">
          <UFormField :label="$t('content.serverName')">
            <UInput v-model="newServer.name" :placeholder="$t('content.serverNamePlaceholder')" class="w-full" autofocus />
          </UFormField>
          <UFormField :label="$t('content.serverAddress')">
            <UInput v-model="newServer.ip" placeholder="mc.example.com" class="w-full" @keydown.enter="addServer" />
          </UFormField>
        </div>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton variant="ghost" color="neutral" :label="$t('common.cancel')" @click="addServerOpen = false" />
          <UButton :label="$t('common.add')" :loading="addingServer" :disabled="!newServer.ip.trim()" @click="addServer" />
        </div>
      </template>
    </UModal>

    <!-- screenshot lightbox -->
    <UModal v-model:open="lightboxOpen" :ui="{ content: 'max-w-5xl w-[92vw]' }">
      <template #content>
        <div v-if="lightbox" class="flex flex-col">
          <div class="flex items-center justify-between gap-2 border-b border-default px-4 py-2.5">
            <span class="truncate text-sm font-medium" :title="lightbox.name">
              {{ lightbox.name }}
              <span class="ml-1 text-xs text-muted">{{ (lightboxIndex ?? 0) + 1 }} / {{ screenshots.length }}</span>
            </span>
            <div class="flex items-center gap-1.5">
              <UButton icon="i-lucide-download" size="xs" color="neutral" variant="soft" :label="$t('content.download')" @click="downloadShot(lightbox)" />
              <UButton icon="i-lucide-folder-open" size="xs" color="neutral" variant="soft" :label="$t('content.openLocation')" @click="revealShot(lightbox)" />
              <UButton icon="i-lucide-trash-2" size="xs" color="error" variant="soft" :label="$t('common.remove')" @click="deleteScreenshot(lightbox)" />
              <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" square @click="lightbox = null" />
            </div>
          </div>

          <div class="relative bg-black/40">
            <img :src="assetUrl(lightbox.path)" class="max-h-[78vh] w-full object-contain" :alt="lightbox.name" >
            <UButton
              v-if="screenshots.length > 1"
              icon="i-lucide-chevron-left"
              color="neutral"
              class="absolute top-1/2 left-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
              @click="step(-1)"
            />
            <UButton
              v-if="screenshots.length > 1"
              icon="i-lucide-chevron-right"
              color="neutral"
              class="absolute top-1/2 right-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
              @click="step(1)"
            />
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { save, confirm } from '@tauri-apps/plugin-dialog'
import type { ScreenshotInfo, WorldInfo, ServerInfo, PingResult } from '~/types/launcher'

/** Worlds, screenshots and servers — mods and packs live in <InstanceContent>. */
type GameFileTab = 'screenshots' | 'worlds' | 'servers'

const props = defineProps<{ instanceId: string; tab: GameFileTab }>()
const emit = defineEmits<{
  quickPlay: [payload: { kind: 'Singleplayer'; world: string } | { kind: 'Multiplayer'; host: string; port?: number }]
}>()
const { t } = useI18n()
const instances = useInstancesStore()

const screenshots = ref<ScreenshotInfo[]>([])
const worlds = ref<WorldInfo[]>([])
const servers = ref<ServerInfo[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const COMMANDS: Record<GameFileTab, string> = {
  screenshots: 'list_screenshots',
  worlds: 'list_worlds',
  servers: 'list_servers',
}

const EMPTY_ICONS: Record<GameFileTab, string> = {
  screenshots: 'i-lucide-camera',
  worlds: 'i-lucide-globe',
  servers: 'i-lucide-server',
}
const emptyIcon = computed(() => EMPTY_ICONS[props.tab])

const count = computed(() => {
  switch (props.tab) {
    case 'screenshots': return screenshots.value.length
    case 'worlds': return worlds.value.length
    case 'servers': return servers.value.length
    default: return 0
  }
})

async function load() {
  loading.value = true
  error.value = null
  try {
    const result = await invoke<unknown>(COMMANDS[props.tab], { id: props.instanceId })
    switch (props.tab) {
      case 'screenshots': screenshots.value = result as ScreenshotInfo[]; break
      case 'worlds': worlds.value = result as WorldInfo[]; break
      case 'servers':
        servers.value = result as ServerInfo[]
        // Reset pings for the new server list and start pinging.
        pings.value = {}
        pingAll()
        break
    }
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

const assetUrl = (path: string) => convertFileSrc(path)
const formatDate = (ms: number) => new Date(ms).toLocaleDateString()

// --- Quick Play helpers ---
const instance = computed(() => instances.instances.find(i => i.id === props.instanceId))

/** MC 1.20+ supports --quickPlay* args */
function supportsQuickPlay(ver: string): boolean {
  const [, minorStr = '0'] = ver.split('.')
  return parseInt(minorStr) >= 20
}
const instanceSupportsQuickPlay = computed(() =>
  !!instance.value && supportsQuickPlay(instance.value.mc_version),
)

// Whether the instance is currently running (disables quick play buttons).
const mc = useMinecraftLaunch(computed(() => props.instanceId))
const isRunning = computed(() => mc.stage.value !== 'idle')

function quickPlayWorld(folder: string) {
  emit('quickPlay', { kind: 'Singleplayer', world: folder })
}
function quickPlayServer(ip: string) {
  const [host, portStr] = ip.split(':')
  const port = portStr ? Number(portStr) : undefined
  emit('quickPlay', { kind: 'Multiplayer', host: host ?? ip, port: Number.isFinite(port) ? port : undefined })
}

// --- servers: add / remove ---
const toast = useToast()
const addServerOpen = ref(false)
const addingServer = ref(false)
const newServer = reactive({ name: '', ip: '' })

// --- server pings ---
type PingStatus = PingResult | 'loading' | 'offline'
const pings = ref<Record<string, PingStatus>>({})

function pingFor(ip: string): PingResult | null {
  const v = pings.value[ip]
  return v && typeof v === 'object' ? v : null
}
function pingStatus(ip: string): 'loading' | 'online' | 'offline' | null {
  const v = pings.value[ip]
  if (!v) return null
  if (v === 'loading') return 'loading'
  if (v === 'offline') return 'offline'
  return 'online'
}

function latencyClass(ms: number): string {
  if (ms < 80) return 'text-emerald-400'
  if (ms < 200) return 'text-yellow-400'
  return 'text-red-400'
}

async function pingAll() {
  for (const s of servers.value) {
    if (pings.value[s.ip] === 'loading') continue
    pings.value = { ...pings.value, [s.ip]: 'loading' }
    invoke<PingResult>('ping_server', { host: s.ip.split(':')[0], port: s.ip.includes(':') ? Number(s.ip.split(':')[1]) : null })
      .then(result => { pings.value = { ...pings.value, [s.ip]: result } })
      .catch(() => { pings.value = { ...pings.value, [s.ip]: 'offline' } })
  }
}

async function addServer() {
  if (!newServer.ip.trim()) return
  addingServer.value = true
  try {
    await invoke('add_server', { id: props.instanceId, name: newServer.name.trim(), ip: newServer.ip.trim() })
    addServerOpen.value = false
    newServer.name = ''
    newServer.ip = ''
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    addingServer.value = false
  }
}

async function removeServer(index: number) {
  try {
    await invoke('delete_server', { id: props.instanceId, index })
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}


// --- worlds: backup / delete ---
const busyWorld = ref<string | null>(null)

async function backupWorld(w: WorldInfo) {
  const dest = await save({ defaultPath: `${w.folder}.zip`, filters: [{ name: 'Zip', extensions: ['zip'] }] })
  if (typeof dest !== 'string') return
  busyWorld.value = w.folder
  try {
    await invoke('backup_world', { id: props.instanceId, folder: w.folder, dest })
    toast.add({ title: t('content.backupDone'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    busyWorld.value = null
  }
}

async function deleteWorld(w: WorldInfo) {
  const ok = await confirm(t('content.deleteWorldConfirm', { name: w.name }), { title: t('content.deleteWorldTitle'), kind: 'warning' })
  if (!ok) return
  try {
    await invoke('delete_world', { id: props.instanceId, folder: w.folder })
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

// --- screenshots: delete ---
async function deleteScreenshot(s: ScreenshotInfo) {
  const ok = await confirm(t('content.deleteScreenshotConfirm'), { title: t('content.deleteScreenshotTitle'), kind: 'warning' })
  if (!ok) return
  try {
    await invoke('delete_screenshot', { id: props.instanceId, name: s.name })
    lightboxIndex.value = null
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

// --- screenshots: lightbox / navigation / download / reveal ---
const lightboxIndex = ref<number | null>(null)
const lightbox = computed<ScreenshotInfo | null>({
  get: () => (lightboxIndex.value !== null ? screenshots.value[lightboxIndex.value] ?? null : null),
  set: (v) => { if (v === null) lightboxIndex.value = null },
})
const lightboxOpen = computed({
  get: () => lightboxIndex.value !== null,
  set: (v: boolean) => { if (!v) lightboxIndex.value = null },
})

/** Moves through the gallery, wrapping around. */
function step(delta: number) {
  if (lightboxIndex.value === null || !screenshots.value.length) return
  const n = screenshots.value.length
  lightboxIndex.value = (lightboxIndex.value + delta + n) % n
}

function onKey(e: KeyboardEvent) {
  if (lightboxIndex.value === null) return
  if (e.key === 'ArrowRight') { e.preventDefault(); step(1) }
  else if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1) }
  else if (e.key === 'Escape') lightboxIndex.value = null
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

async function downloadShot(s: ScreenshotInfo) {
  try {
    const dest = await save({ defaultPath: s.name, filters: [{ name: 'PNG', extensions: ['png'] }] })
    if (!dest) return
    await invoke('copy_file', { from: s.path, to: dest })
    toast.add({ title: t('content.downloaded'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function revealShot(s: ScreenshotInfo) {
  try {
    await invoke('reveal_in_explorer', { path: s.path })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

watch(() => [props.instanceId, props.tab], load, { immediate: true })
</script>
