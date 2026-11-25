<template>
  <div v-if="instance" class="h-full overflow-y-auto">
    <div class="mx-auto max-w-[1400px] space-y-6 px-8 pb-10 pt-7">
      <button
        type="button"
        class="-ml-1 flex items-center gap-1 rounded-md px-1 text-[13px] text-muted transition-colors hover:text-highlighted"
        @click="router.push('/')"
      >
        <UIcon name="i-lucide-chevron-left" class="size-4" /> {{ $t('instance.back') }}
      </button>

      <div ref="headerEl">
        <div class="flex flex-wrap items-center gap-5">
          <InstanceIcon :key="iconKey" :instance="instance" class="size-[72px] rounded-xl text-3xl" />
          <div class="min-w-0 flex-1">
            <h1 class="truncate text-[22px] font-semibold tracking-[-0.01em] text-highlighted">{{ instance.name }}</h1>
            <div class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] tabular-nums text-muted">
              <span class="inline-flex items-center gap-1.5">
                <UIcon name="i-lucide-gamepad-2" class="size-3.5 text-dimmed" />
                {{ loaderLabel(instance.loader.type) }} {{ instance.mc_version }}<template v-if="loaderVersion"> · {{ loaderVersion }}</template>
              </span>
              <span class="inline-flex items-center gap-1.5">
                <UIcon name="i-lucide-clock" class="size-3.5 text-dimmed" />
                {{ instance.playtime_seconds ? playtime : $t('instance.neverPlayed') }}
              </span>
              <span v-if="instance.last_played" class="inline-flex items-center gap-1.5">
                <UIcon name="i-lucide-calendar" class="size-3.5 text-dimmed" />
                {{ lastPlayed }}
              </span>
              <UBadge v-if="instance.group" color="neutral" variant="soft" size="sm" :label="instance.group" />
            </div>
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <AddonSlot name="instance.header" :context="{ instanceId: id }" />
            <UButton
              icon="i-lucide-folder-open"
              color="neutral"
              variant="soft"
              size="lg"
              square
              :title="$t('instance.openGameFolder')"
              :aria-label="$t('instance.openGameFolder')"
              @click="openGameFolder"
            />
            <UButton
              icon="i-lucide-settings"
              color="neutral"
              variant="soft"
              size="lg"
              square
              :title="$t('instance.tabs.settings')"
              :aria-label="$t('instance.tabs.settings')"
              @click="settingsOpen = true"
            />
            <UDropdownMenu :items="menuItems">
              <UButton icon="i-lucide-ellipsis" color="neutral" variant="soft" size="lg" square aria-label="…" />
            </UDropdownMenu>
            <template v-if="mc.stage.value === 'running'">
              <UButton
                icon="i-lucide-square"
                color="neutral"
                variant="soft"
                size="lg"
                :loading="stopping"
                :label="$t('instance.close')"
                :title="$t('instance.closeHint')"
                @click="stopInstance(false)"
              />
              <UButton
                icon="i-lucide-skull"
                color="error"
                variant="soft"
                size="lg"
                :loading="killing"
                :label="$t('instance.kill')"
                :title="$t('instance.killHint')"
                @click="stopInstance(true)"
              />
            </template>
            <UButton
              v-else
              size="lg"
              :disabled="isBusy"
              :loading="isBusy"
              :label="playLabel"
              class="px-6"
              @click="play"
            >
              <template #leading>
                <svg v-if="!isBusy" class="size-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l13-7.5z" /></svg>
              </template>
            </UButton>
          </div>
        </div>

        <div v-if="mc.stage.value !== 'idle' && mc.stage.value !== 'running'" class="mt-5 space-y-2">
          <div class="flex items-center justify-between text-[13px] text-muted">
            <span>{{ stageLabel }}</span>
            <span v-if="mc.stage.value === 'installing' && mc.progress.value.total" class="tabular-nums">
              {{ mc.progress.value.current }} / {{ mc.progress.value.total }}
            </span>
          </div>
          <UProgress
            size="sm"
            :model-value="mc.stage.value === 'installing' ? mc.progress.value.current : undefined"
            :max="mc.progress.value.total || 100"
          />
        </div>

        <p v-if="mc.error.value" class="mt-3 text-sm text-error">{{ mc.error.value }}</p>

        <div v-if="modpackUpdate" class="mt-5 rounded-xl border border-primary/40 bg-primary/8 p-3.5">
          <div class="flex flex-wrap items-center gap-2">
            <UIcon name="i-lucide-circle-arrow-up" class="size-4 text-primary" />
            <span class="text-sm font-medium text-highlighted">{{ $t('instance.updateAvailable', { v: modpackUpdate.version_number }) }}</span>
            <UBadge v-if="modpackUpdate.version_type === 'beta'" color="warning" variant="subtle" size="xs" label="BETA" />
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
          <div v-if="showChangelog && changelogHtml" class="mk-md mt-3 max-h-64 overflow-y-auto border-t border-primary/20 pt-3 text-sm" v-html="changelogHtml" />
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

      <div class="sticky top-0 z-20 -mx-8 border-b border-default bg-default px-8">
        <div class="flex items-center gap-1">
          <Transition
            enter-active-class="transition duration-150 ease-out"
            enter-from-class="opacity-0 -translate-x-1"
            leave-active-class="transition duration-100 ease-in"
            leave-to-class="opacity-0"
          >
            <div v-if="compact" class="mr-2 flex min-w-0 shrink items-center gap-2 border-r border-default py-1.5 pr-3">
              <InstanceIcon :instance="instance" class="size-6 rounded-md text-[11px]" />
              <span class="max-w-40 truncate text-sm font-semibold text-highlighted">{{ instance.name }}</span>
            </div>
          </Transition>
          <div class="flex min-w-0 flex-1 gap-1 overflow-x-auto [scrollbar-width:none]" role="tablist">
            <button
              v-for="tab in tabs"
              :key="tab.key"
              type="button"
              role="tab"
              :aria-selected="activeTab === tab.key && !addonTab"
              class="-mb-px flex shrink-0 items-center gap-1.5 border-b-2 px-3 pb-2.5 pt-2.5 text-sm font-medium transition-colors"
              :class="[
                activeTab === tab.key && !addonTab
                  ? 'border-primary text-highlighted'
                  : 'border-transparent text-muted hover:text-highlighted',
                tab.key === firstManageTab ? 'ml-auto' : '',
              ]"
              :title="$t(tab.label)"
              @click="selectTab(tab.key)"
            >
              <UIcon :name="tab.icon" class="size-4" />
              <span :class="{ 'sr-only': compact && MANAGE_TABS.includes(tab.key) }">{{ $t(tab.label) }}</span>
            </button>
            <button
              v-for="tab in addonTabs"
              :key="tab.key"
              type="button"
              role="tab"
              :aria-selected="addonTab === tab.key"
              class="-mb-px flex shrink-0 items-center gap-1.5 border-b-2 px-3 pb-2.5 pt-2.5 text-sm font-medium transition-colors"
              :class="addonTab === tab.key
                ? 'border-primary text-highlighted'
                : 'border-transparent text-muted hover:text-highlighted'"
              @click="addonTab = tab.key"
            >
              <img v-if="tab.icon" :src="convertFileSrc(tab.icon)" alt="" class="size-4 object-contain">
              <UIcon v-else name="i-lucide-puzzle" class="size-4" />
              {{ tab.label }}
            </button>
          </div>
          <Transition
            enter-active-class="transition duration-150 ease-out"
            enter-from-class="opacity-0 translate-x-1"
            leave-active-class="transition duration-100 ease-in"
            leave-to-class="opacity-0"
          >
            <div v-if="compact" class="ml-2 shrink-0 py-1.5">
              <UButton
                v-if="mc.stage.value === 'running'"
                icon="i-lucide-square"
                color="neutral"
                variant="soft"
                size="sm"
                :loading="stopping"
                :label="$t('instance.close')"
                @click="stopInstance(false)"
              />
              <UButton v-else size="sm" :disabled="isBusy" :loading="isBusy" :label="playLabel" @click="play">
                <template #leading>
                  <svg v-if="!isBusy" class="size-3.5" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l13-7.5z" /></svg>
                </template>
              </UButton>
            </div>
          </Transition>
        </div>
      </div>

      <AddonFrame
        v-if="currentAddonTab"
        :key="`${currentAddonTab.key}@${id}`"
        :addon-id="currentAddonTab.addonId"
        :entry="currentAddonTab.entry"
        :context="{ instanceId: id }"
        :title="currentAddonTab.label"
        class="h-[70vh] w-full rounded-xl border border-default"
      />

      <div v-else>
        <InstanceLogs v-if="activeTab === 'logs'" :instance-id="id" :initial-rel="initialCrashRel" />

        <InstanceContent v-else-if="activeTab === 'content'" :instance-id="id" :initial-kind="initialKind" />

        <InstanceGameFiles v-else-if="isGameFileTab" :instance-id="id" :tab="activeTab as GameFileTab" @quick-play="handleQuickPlay" />

        <InstanceConfigs v-else-if="activeTab === 'configs'" :instance-id="id" />

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
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import type { ModpackUpdate } from '~/types/modrinth'
import type { QuickPlay } from '~/types/launcher'

const route = useRoute()
const router = useRouter()
const instances = useInstancesStore()
const accounts = useAccountStore()
const sysMem = useSystemMemory()
const toast = useToast()
const { t, locale } = useI18n()
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
    toast.add({ title: errorText(e), color: 'error' })
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
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    stopping.value = false
    killing.value = false
  }
}

type TabKey = 'content' | 'worlds' | 'screenshots' | 'servers' | 'configs' | 'logs' | 'share' | 'snapshots'
const tabs: { key: TabKey; label: string; icon: string }[] = [
  { key: 'content', label: 'instance.tabs.content', icon: 'i-lucide-blocks' },
  { key: 'worlds', label: 'instance.tabs.worlds', icon: 'i-lucide-globe' },
  { key: 'servers', label: 'instance.tabs.servers', icon: 'i-lucide-server' },
  { key: 'screenshots', label: 'instance.tabs.screenshots', icon: 'i-lucide-camera' },
  { key: 'logs', label: 'instance.tabs.logs', icon: 'i-lucide-scroll-text' },
  { key: 'configs', label: 'instance.tabs.configs', icon: 'i-lucide-sliders-horizontal' },
  { key: 'snapshots', label: 'instance.tabs.snapshots', icon: 'i-lucide-history' },
  { key: 'share', label: 'instance.tabs.share', icon: 'i-lucide-share-2' },
]
const MANAGE_TABS: TabKey[] = ['configs', 'snapshots', 'share']
const firstManageTab = MANAGE_TABS[0]

// The open tab lives in the URL, so coming back to an instance returns to the same tab.
function selectTab(key: TabKey) {
  activeTab.value = key
  addonTab.value = null
  router.replace({ query: { ...route.query, tab: key === 'content' ? undefined : key } })
}

const headerEl = ref<HTMLElement | null>(null)
const compact = ref(false)
let headerObserver: IntersectionObserver | null = null
onMounted(() => {
  headerObserver = new IntersectionObserver(([e]) => { compact.value = !e!.isIntersecting }, { threshold: 0 })
  watchEffect(() => {
    headerObserver?.disconnect()
    if (headerEl.value) headerObserver?.observe(headerEl.value)
  })
})
onBeforeUnmount(() => headerObserver?.disconnect())
const activeTab = ref<TabKey>('content')
const settingsOpen = ref(false)

const addons = useAddonsStore()
const addonTab = ref<string | null>(null)
const addonTabs = computed(() => addons.active.flatMap(a => a.instanceTabs.map(tab => ({
  key: `${a.id}:${tab.id}`,
  addonId: a.id,
  entry: tab.entry,
  icon: tab.icon,
  label: addons.text(a.id, tab.title, locale.value),
}))))
const currentAddonTab = computed(() => addonTabs.value.find(tab => tab.key === addonTab.value) ?? null)

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
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function openGameFolder() {
  try {
    await invoke('open_instance_game_folder', { id: id.value })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

const iconKey = ref(0)

</script>
