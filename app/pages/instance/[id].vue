<template>
  <div v-if="instance" class="h-full overflow-y-auto">
    <div class="space-y-6 p-6 lg:p-8">
      <button
        type="button"
        class="flex items-center gap-1.5 text-sm text-muted transition hover:text-default"
        @click="router.push('/')"
      >
        <UIcon name="i-lucide-arrow-left" class="size-4" /> {{ $t('instance.back') }}
      </button>

      <div class="relative overflow-hidden rounded-2xl border border-default bg-linear-[135deg] from-primary-500/12 to-transparent p-6">
        <div class="flex flex-wrap items-center gap-5">
          <div class="size-20 shrink-0 overflow-hidden rounded-2xl shadow-[0_8px_24px_rgba(0,0,0,0.4)]">
            <InstanceIcon :key="iconKey" :instance="instance" class="size-full rounded-2xl text-3xl" />
          </div>
          <div class="min-w-0 flex-1">
            <h1 class="truncate text-2xl font-bold tracking-tight">{{ instance.name }}</h1>
            <div class="mt-2 flex flex-wrap items-center gap-2">
              <span class="rounded-md bg-white/7 px-2.5 py-1 font-mono text-xs">Minecraft {{ instance.mc_version }}</span>
              <span
                class="inline-flex items-center rounded-md px-2 py-1 text-xs font-medium"
                :class="loaderBadgeClass(instance.loader.type)"
              >
                {{ loaderLabel(instance.loader.type) }}
                <span v-if="loaderVersion" class="ml-1 font-mono opacity-70">{{ loaderVersion }}</span>
              </span>
              <UBadge v-if="instance.group" color="neutral" variant="soft" size="sm" :label="instance.group" />
            </div>
            <p class="mt-2 text-xs text-muted">
              {{ instance.last_played ? `${$t('instance.lastPlayed')}: ${lastPlayed}` : $t('instance.neverPlayed') }}
              <span v-if="instance.playtime_seconds">· {{ $t('instance.playtime') }}: {{ playtime }}</span>
            </p>
          </div>

          <div class="flex items-center gap-2">
            <UButton
              icon="i-lucide-folder"
              color="neutral"
              variant="soft"
              :label="$t('instance.openGameFolder')"
              @click="openGameFolder"
            />
            <UButton
              icon="i-lucide-settings"
              color="neutral"
              variant="soft"
              square
              :title="$t('instance.tabs.settings')"
              @click="settingsOpen = true"
            />
            <UDropdownMenu :items="menuItems">
              <UButton icon="i-lucide-ellipsis-vertical" color="neutral" variant="soft" square />
            </UDropdownMenu>
            <template v-if="mc.stage.value === 'running'">
              <UButton
                icon="i-lucide-square"
                color="neutral"
                variant="soft"
                :loading="stopping"
                :label="$t('instance.close')"
                :title="$t('instance.closeHint')"
                @click="stopInstance(false)"
              />
              <UButton
                icon="i-lucide-skull"
                color="error"
                variant="soft"
                :loading="killing"
                :label="$t('instance.kill')"
                :title="$t('instance.killHint')"
                @click="stopInstance(true)"
              />
            </template>
            <button
              type="button"
              :disabled="isBusy"
              class="flex items-center justify-center gap-2 rounded-xl bg-[#3fb877] px-7 py-3 text-[15px] font-bold tracking-[0.02em] text-[#06210f] shadow-[0_6px_22px_rgba(63,184,119,0.32)] transition hover:bg-[#4bcb86] active:scale-[0.99] disabled:opacity-60"
              @click="play"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M7 5l13 7-13 7z" /></svg>
              {{ playLabel }}
            </button>
          </div>
        </div>

        <div v-if="mc.stage.value !== 'idle'" class="mt-5 space-y-2">
          <div class="flex items-center justify-between text-xs text-muted">
            <span>{{ stageLabel }}</span>
            <span v-if="mc.stage.value === 'installing' && mc.progress.value.total">
              {{ mc.progress.value.current }} / {{ mc.progress.value.total }}
            </span>
          </div>
          <UProgress
            :model-value="mc.stage.value === 'installing' ? mc.progress.value.current : undefined"
            :max="mc.progress.value.total || 100"
          />
        </div>

        <p v-if="mc.error.value" class="mt-3 text-sm text-error">{{ mc.error.value }}</p>

        <div v-if="modpackUpdate" class="mt-4 rounded-xl border border-primary-500/40 bg-primary-500/10 p-3">
          <div class="flex flex-wrap items-center gap-2">
            <UIcon name="i-lucide-circle-arrow-up" class="size-4 text-primary-400" />
            <span class="text-sm font-medium">{{ $t('instance.updateAvailable', { v: modpackUpdate.version_number }) }}</span>
            <UBadge v-if="modpackUpdate.version_type === 'beta'" color="warning" variant="solid" size="xs" label="BETA" />
            <div class="ml-auto flex items-center gap-2">
              <UButton
                v-if="modpackUpdate.changelog"
                color="neutral"
                variant="ghost"
                size="xs"
                :icon="showChangelog ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
                :label="$t('instance.showChangelog')"
                @click="toggleChangelog"
              />
              <UButton color="primary" size="xs" :loading="updatingModpack" :label="$t('instance.updateNow')" @click="updateModpack" />
            </div>
          </div>
          <div v-if="showChangelog && changelogHtml" class="mk-md mt-3 max-h-64 overflow-y-auto border-t border-primary-500/20 pt-3 text-sm" v-html="changelogHtml" />
        </div>
      </div>

      <UModal
        v-model:open="settingsOpen"
        :title="$t('instance.tabs.settings')"
        :ui="{ content: 'max-w-3xl h-[min(38rem,85vh)]', body: 'flex-1 min-h-0 overflow-hidden p-0' }"
      >
        <template #body>
          <InstanceSettings :instance-id="id" @icon-changed="iconKey++" />
        </template>
      </UModal>

      <ChangeLoaderModal />

      <div class="flex flex-wrap gap-1.5 border-b border-default pb-3">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          type="button"
          class="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition"
          :class="activeTab === tab.key
            ? 'bg-primary-500/15 text-primary-400'
            : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
          @click="activeTab = tab.key"
        >
          <UIcon :name="tab.icon" class="size-4" />
          {{ $t(tab.label) }}
        </button>
      </div>

      <div>
        <InstanceLogs v-if="activeTab === 'logs'" :instance-id="id" :initial-rel="initialCrashRel" />

        <InstanceContent v-else-if="activeTab === 'content'" :instance-id="id" :initial-kind="initialKind" />

        <InstanceGameFiles v-else-if="isGameFileTab" :instance-id="id" :tab="activeTab as GameFileTab" @quick-play="handleQuickPlay" />

        <InstanceShare
          v-else-if="activeTab === 'share'"
          :instance-id="id"
          :instance-name="instance?.name ?? ''"
        />

        <InstanceSnapshots
          v-else-if="activeTab === 'snapshots'"
          :instance-id="id"
          :instance-name="instance?.name ?? ''"
        />
      </div>
    </div>
  </div>

  <div v-else class="flex h-full items-center justify-center text-muted">
    {{ $t('instance.notFound') }}
  </div>

  <UModal v-model:open="prelaunchOpen" :title="$t('prelaunch.title')" :ui="{ content: 'max-w-md' }">
    <template #body>
      <ul class="space-y-1.5 text-sm">
        <li v-for="(w, i) in prelaunchWarnings" :key="i" class="flex items-start gap-2 text-amber-200/90">
          <UIcon name="i-lucide-triangle-alert" class="mt-0.5 size-4 shrink-0 text-amber-400" />
          <span>{{ w }}</span>
        </li>
      </ul>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton variant="ghost" color="neutral" :label="$t('common.cancel')" @click="prelaunchOpen = false" />
        <UButton color="warning" :label="$t('prelaunch.launchAnyway')" @click="confirmLaunch" />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { ModpackUpdate } from '~/types/modrinth'
import type { QuickPlay } from '~/types/launcher'

const route = useRoute()
const router = useRouter()
const instances = useInstancesStore()
const accounts = useAccountStore()
const sysMem = useSystemMemory()
const toast = useToast()
const { t } = useI18n()
const exportModal = useExportModal()
const changeLoaderModal = useChangeLoaderModal()

const id = computed(() => String(route.params.id))
const mc = useMinecraftLaunch(id)
const modrinth = useModrinth()
const activity = useActivityCenter()

onMounted(async () => {
  await instances.ensureLoaded()
  mc.attach()
  checkModpackUpdate()
  try {
    if (await invoke<boolean>('is_instance_running', { id: id.value })) {
      activity.markRunning(id.value)
    }
  } catch {  }
})

const instance = computed(() => instances.instances.find(i => i.id === id.value))

const modpackUpdate = ref<ModpackUpdate | null>(null)
const showChangelog = ref(false)
const updatingModpack = ref(false)
const changelogHtml = ref('')

async function checkModpackUpdate() {
  modpackUpdate.value = null
  showChangelog.value = false
  if (!instance.value?.modpack_project_id) return
  try {
    modpackUpdate.value = await modrinth.checkModpackUpdate(id.value)
  } catch {  }
}

async function toggleChangelog() {
  showChangelog.value = !showChangelog.value
  if (showChangelog.value && !changelogHtml.value && modpackUpdate.value?.changelog) {
    const { marked } = await import('marked')
    const raw = marked.parse(modpackUpdate.value.changelog, { async: false }) as string
    changelogHtml.value = (await import('dompurify')).default.sanitize(raw)
  }
}

async function updateModpack() {
  updatingModpack.value = true
  const tid = activity.startTask(t('activity.updatingModpack'))
  try {
    await modrinth.updateModpack(id.value)
    await instances.load()
    toast.add({ title: t('instance.updated'), color: 'success' })
    modpackUpdate.value = null
    showChangelog.value = false
    changelogHtml.value = ''
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(tid)
    updatingModpack.value = false
  }
}

const loaderVersion = computed(() =>
  instance.value && 'version' in instance.value.loader ? instance.value.loader.version : '',
)

const lastPlayed = computed(() =>
  instance.value?.last_played ? new Date(instance.value.last_played).toLocaleDateString() : '',
)

const playtime = computed(() => {
  const s = instance.value?.playtime_seconds ?? 0
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  return h ? `${h} h ${m} min` : `${m} min`
})

const isBusy = computed(() => mc.launching.value || mc.stage.value !== 'idle')
const stageLabel = computed(() => (mc.stage.value === 'installing' ? t('common.loading') : t('instance.running')))
const playLabel = computed(() => {
  if (mc.stage.value === 'installing') return t('common.loading')
  if (mc.stage.value === 'running') return t('instance.running')
  return t('instance.play')
})
const stopping = ref(false)
const killing = ref(false)

async function stopInstance(force: boolean) {
  if (force) killing.value = true
  else stopping.value = true
  try {
    await invoke('stop_instance', { id: id.value, force })
    try {
      if (!(await invoke<boolean>('is_instance_running', { id: id.value }))) {
        activity.clear(id.value)
      }
    } catch {  }
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    stopping.value = false
    killing.value = false
  }
}

type TabKey = 'content' | 'worlds' | 'screenshots' | 'servers' | 'logs' | 'share' | 'snapshots'
const tabs: { key: TabKey; label: string; icon: string }[] = [
  { key: 'content', label: 'instance.tabs.content', icon: 'i-lucide-blocks' },
  { key: 'worlds', label: 'instance.tabs.worlds', icon: 'i-lucide-globe' },
  { key: 'screenshots', label: 'instance.tabs.screenshots', icon: 'i-lucide-camera' },
  { key: 'servers', label: 'instance.tabs.servers', icon: 'i-lucide-server' },
  { key: 'logs', label: 'instance.tabs.logs', icon: 'i-lucide-scroll-text' },
  { key: 'share', label: 'instance.tabs.share', icon: 'i-lucide-share-2' },
  { key: 'snapshots', label: 'instance.tabs.snapshots', icon: 'i-lucide-history' },
]
const activeTab = ref<TabKey>('content')
const settingsOpen = ref(false)

type ContentTabKind = 'mod' | 'resourcepack' | 'shader' | 'datapack'
const MERGED_TABS: Record<string, ContentTabKind> = {
  mods: 'mod',
  resourcepacks: 'resourcepack',
  shaders: 'shader',
  datapacks: 'datapack',
}

const initialCrashRel = ref<string | null>(null)
const initialKind = ref<ContentTabKind | undefined>()
onMounted(() => {
  const tabParam = route.query.tab as string | undefined
  if (tabParam && tabs.some(t => t.key === tabParam)) {
    activeTab.value = tabParam as TabKey
  } else if (tabParam && MERGED_TABS[tabParam]) {
    activeTab.value = 'content'
    initialKind.value = MERGED_TABS[tabParam]
  }
  const crashRelParam = route.query.crashRel as string | undefined
  if (crashRelParam) {
    initialCrashRel.value = decodeURIComponent(crashRelParam)
    router.replace({ query: {} })
  }
})

type GameFileTab = 'screenshots' | 'worlds' | 'servers'
const GAME_FILE_TABS: GameFileTab[] = ['screenshots', 'worlds', 'servers']
const isGameFileTab = computed(() => (GAME_FILE_TABS as string[]).includes(activeTab.value))

const menuItems = computed(() => [[
  {
    label: t('ctx.openFolder'),
    icon: 'i-lucide-folder',
    onSelect: openGameFolder,
  },
  {
    label: t('instance.export'),
    icon: 'i-lucide-package',
    onSelect: () => { if (instance.value) exportModal.open(id.value, instance.value.name) },
  },
  {
    label: t('changeLoader.menu'),
    icon: 'i-lucide-layers',
    onSelect: () => changeLoaderModal.open(id.value),
  },
  {
    label: t('instance.createShortcut'),
    icon: 'i-lucide-app-window',
    onSelect: createShortcut,
  },
], [
  {
    label: t('common.remove'),
    icon: 'i-lucide-trash-2',
    color: 'error' as const,
    onSelect: async () => {
      await instances.remove(id.value)
      router.push('/')
    },
  },
]])

const prelaunchOpen = ref(false)
const prelaunchWarnings = ref<string[]>([])
let pendingQuickPlay: QuickPlay | undefined

async function collectWarnings(): Promise<string[]> {
  const out: string[] = []
  const inst = instance.value
  if (!inst) return out
  await sysMem.ensure()
  if (inst.override_memory && inst.memory_mb && sysMem.totalMb.value && inst.memory_mb > sysMem.totalMb.value * 0.9) {
    out.push(t('prelaunch.ramHigh', { mb: inst.memory_mb }))
  }
  try {
    const conflicts = await invoke<{ name: string, kind: string, detail: string }[]>('check_conflicts', { instanceId: id.value })
    for (const c of conflicts) {
      out.push(c.kind === 'loader' ? t('prelaunch.conflict', { name: c.name, detail: c.detail }) : t('prelaunch.duplicate', { name: c.name }))
    }
  } catch {  }
  return out
}

async function launchWith(qp?: QuickPlay) {
  if (!instance.value) return
  if (!accounts.activeAccount) {
    toast.add({ title: t('prelaunch.noAccount'), color: 'error' })
    return
  }
  const warnings = await collectWarnings()
  if (warnings.length) {
    prelaunchWarnings.value = warnings
    pendingQuickPlay = qp
    prelaunchOpen.value = true
    return
  }
  doLaunch(qp)
}

function doLaunch(qp?: QuickPlay) {
  if (!instance.value) return
  mc.launch(instance.value.id, qp).catch(() => {  })
}

function confirmLaunch() {
  prelaunchOpen.value = false
  doLaunch(pendingQuickPlay)
  pendingQuickPlay = undefined
}

const play = () => launchWith()
const handleQuickPlay = (qp: QuickPlay) => launchWith(qp)

async function createShortcut() {
  try {
    await invoke<string>('create_desktop_shortcut', { id: id.value })
    toast.add({ title: t('instance.shortcutCreated'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function openGameFolder() {
  try {
    await invoke('open_instance_game_folder', { id: id.value })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

const iconKey = ref(0)

</script>
