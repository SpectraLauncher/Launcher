<template>
  <UApp class="overflow-hidden">
    <Teleport to="body">
      <div
        data-tauri-drag-region
        class="pointer-events-auto fixed inset-x-0 top-0 z-[100] flex justify-between items-center h-10 px-2 text-gray-100 select-none"
        @pointerdown.self.stop
      >
        <template v-if="isMac">
          <!-- The system draws its traffic lights over this corner; the spacer
               keeps the right-hand group where justify-between expects it. -->
          <div class="w-[72px]" />
          <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-2">
            <img src="/logo-transparent.png" alt="Spectra Launcher Icon" class="h-5 object-contain" />
            <span>{{ windowTitle }}</span>
          </div>
          <div class="flex items-center gap-3">
            <template v-if="!isContentWindow && !booting">
              <AddonSlot name="titlebar" variant="icon" />
              <TitlebarActivity />
              <AccountButton />
            </template>
          </div>
        </template>
        <template v-else>
          <div class="flex items-center gap-2 pl-2">
            <img src="/logo-transparent.png" alt="Spectra Launcher Icon" class="h-5 object-contain" />
            <span>{{ windowTitle }}</span>
          </div>
          <div class="flex items-center gap-3">
            <template v-if="!isContentWindow && !booting">
              <AddonSlot name="titlebar" variant="icon" />
              <TitlebarActivity />
              <AccountButton />
            </template>
            <WindowControls />
          </div>
        </template>
      </div>

      <NuxtLoadingIndicator color="aqua" errorColor="red" />
    </Teleport>

    <div class="h-10" />

    <div :class="['relative w-screen h-[calc(100vh-2.5rem)] overflow-hidden text-[#eef1f5]', theme.bgClass]">

      <div
        v-if="themeBackground"
        class="pointer-events-none absolute inset-0 bg-cover bg-center opacity-30"
        :style="{ backgroundImage: `url('${themeBackground}')` }"
      />

      <div
        class="pointer-events-none absolute inset-0"
        style="background-image:radial-gradient(rgba(255,255,255,0.035) 1px,transparent 1px);background-size:26px 26px;"
      />

      <div class="relative z-[1] flex h-full flex-col">
        <div
          v-if="!isContentWindow && spectra.user.value?.emailVerified === false"
          role="status"
          class="flex shrink-0 flex-wrap items-center justify-center gap-x-3 gap-y-1 border-b border-amber-400/25 bg-amber-400/10 px-4 py-2 text-center text-xs text-amber-100"
        >
          <UIcon name="i-lucide-mail-warning" class="size-4 shrink-0" />
          <span>{{ t('spectra.verifyEmailWarning') }}</span>
          <button type="button" class="font-semibold underline underline-offset-2 hover:text-white" @click="openAccountSettings">
            {{ t('spectra.verifyEmailAction') }}
          </button>
        </div>

        <div class="relative min-h-0 flex-1">
          <NuxtLayout>
            <NuxtPage />
          </NuxtLayout>

          <AccountSidebar v-if="!isContentWindow" />
        </div>
      </div>
    </div>

    <template v-if="!isContentWindow">
      <AddonMains />
      <CrashReportModal />
      <StartupUpdate />
      <CloudSyncConflictModal />
    </template>
  </UApp>
</template>

<script setup lang="ts">
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { emitTo, listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

const { t, locale } = useI18n()
const theme = useThemeStore()
const toast = useToast()

const route = useRoute()
const isAddonWindow = computed(() => route.path.startsWith('/addon-window'))
const isConsoleWindow = computed(() => route.path.startsWith('/console'))
const isContentWindow = computed(() => route.path.startsWith('/browser') || isAddonWindow.value || isConsoleWindow.value)
const windowTitle = computed(() => {
  if (isAddonWindow.value) {
    const addon = addons.active.find(a => a.id === route.query.addon)
    const win = addon?.windows.find(w => w.id === route.query.window)
    return addon && win ? `${win.title} — ${addon.name}` : 'Spectra'
  }
  if (isConsoleWindow.value) return t('logs.window')
  return isContentWindow.value ? 'Spectra — content' : 'Spectra Launcher'
})

const { platform } = usePlatform()
const isMac = computed(() => platform.value === 'macos')

const activity = useActivityCenter()
const instances = useInstancesStore()
const router = useRouter()
const mc = useMinecraftLaunch()
const updater = useAutoUpdate()
const booting = computed(() => !isContentWindow.value && updater.gate.value !== 'done')

// The content browser is a second window running the same app. Only the main
// window owns the update — otherwise opening the browser would kick off its own
// check and could restart the launcher out from under someone mid-browse.
const bootGate = () => (isContentWindow.value ? Promise.resolve() : updater.updateOnStartup())
const telemetry = useTelemetry()
const createModal = useCreateInstanceModal()
const spectra = useSpectraAccount()
const spectraNotifications = useSpectraNotifications()
const cloud = useCloudSync()
const addons = useAddonsStore()
const addonInstall = useAddonInstall()

initAddonBridge({
  toast: value => toast.add(value),
  navigate: (addonId, page) => {
    if (isContentWindow.value) emitTo('main', 'addon://navigate', { addonId, page }).catch(() => {})
    else router.push(`/addon/${addonId}/${page}`)
  },
  launch: instanceId => (isContentWindow.value ? emitTo('main', 'addon://launch', instanceId) : mc.launch(instanceId)),
  locale: () => locale.value,
  theme: () => ({ mode: theme.mode, accent: theme.accent }),
  addonName: id => addons.addons.find(a => a.id === id)?.name ?? id,
  openUrl: url => openExternal(url),
  t: (key, params) => t(key, params ?? {}),
})

let knownInstances: Set<string> | null = null
watch(() => (instances.loaded ? instances.instances.map(i => i.id) : null), (ids) => {
  if (!ids || isContentWindow.value) return
  if (knownInstances) {
    for (const id of ids) {
      if (!knownInstances.has(id)) broadcastAddonEvent('instance:created', { instanceId: id })
    }
    for (const id of knownInstances) {
      if (!ids.includes(id)) broadcastAddonEvent('instance:removed', { instanceId: id })
    }
  }
  knownInstances = new Set(ids)
})

const themeBackground = computed(() => {
  const picked = addons.themes.find(t => t.key === theme.addonTheme)
  return picked?.background ? convertFileSrc(picked.background) : null
})

async function openAccountSettings() {
  const url = await invoke<string>('spectra_account_settings_url')
  await openExternal(url)
}

function refreshAccount() {
  if (spectra.isSignedIn.value) spectra.refresh()
}

let accountRefreshTimer: ReturnType<typeof setInterval> | null = null
let cloudTimer: ReturnType<typeof setInterval> | null = null

const contentWindow = useContentWindow()

let unlistenContent: UnlistenFn | null = null
let unlistenShare: UnlistenFn | null = null
let unlistenAccount: UnlistenFn | null = null
let unlistenLaunch: UnlistenFn | null = null
let unlistenCloudExit: UnlistenFn | null = null
let unlistenAddon: UnlistenFn | null = null
const addonUnlisteners: UnlistenFn[] = []
onMounted(async () => {
  await bootGate()

  addons.load().catch(() => {})
  await addons.checkAvailable()
  if (!isContentWindow.value && addons.available) {
    addons.checkRevoked()
      .then((names) => {
        if (names.length) toast.add({ title: t('addons.revokedToast', { names: names.join(', ') }), color: 'error' })
      })
      .catch(() => {})
    addons.checkUpdates()
      .then((updates) => {
        if (updates.length) toast.add({ title: t('addons.updatesFound', { n: updates.length }), color: 'info' })
      })
      .catch(() => {})
  }
  addonUnlisteners.push(
    await listen<{ name: string; payload: Record<string, unknown> }>('addon://event', (e) => {
      emitAddonEvent(e.payload.name, e.payload.payload)
    }),
    await listen<{ instance_id: string; code: number | null }>('mc://exited', (e) => {
      emitAddonEvent('game:exit', { instanceId: e.payload.instance_id, code: e.payload.code })
      if (!isContentWindow.value) void instances.load()
    }),
  )
  if (!isContentWindow.value) {
    addonUnlisteners.push(
      await listen<{ addonId: string; page: string }>('addon://navigate', (e) => {
        router.push(`/addon/${e.payload.addonId}/${e.payload.page}`)
      }),
      await listen<string>('addon://launch', (e) => {
        mc.launch(e.payload).catch(() => {})
      }),
    )
    unlistenAddon = await listen<string>('addon://open', async (e) => {
      await invoke('take_pending_addon').catch(() => {})
      if (await addons.checkAvailable()) addonInstall.fromCatalog(e.payload)
    })
    const pendingAddon = await invoke<string | null>('take_pending_addon')
    if (pendingAddon && addons.available) addonInstall.fromCatalog(pendingAddon)
  }

  unlistenShare = await listen<string>('share://open', async (e) => {
    await invoke('take_pending_share').catch(() => {})
    createModal.openWithCode(e.payload)
  })
  const pending = await invoke<string | null>('take_pending_share')
  if (pending) createModal.openWithCode(pending)

  unlistenLaunch = await listen<string>('launch://open', async (e) => {
    await invoke('take_pending_launch').catch(() => {})
    playInstance(e.payload)
  })
  const pendingLaunch = await invoke<string | null>('take_pending_launch')
  if (pendingLaunch) playInstance(pendingLaunch)

  if (!isContentWindow.value) {
    unlistenCloudExit = await listen('mc://exited', () => { void cloud.tick().catch(() => {}) })
  }

  unlistenAccount = await listen('spectra://account', async () => {
    await spectra.refresh()
    addons.checkAvailable()
    void cloud.tick().catch(() => {})
    spectraNotifications.start()
    spectra.linkMinecraft()
  })

  unlistenContent = await contentWindow.onInstalled(async ({ instance }) => {
    await instances.load()
    if (instance) router.push(`/instance/${instance.id}`)
  })
})
onBeforeUnmount(() => {
  unlistenShare?.()
  unlistenAccount?.()
  unlistenLaunch?.()
  unlistenCloudExit?.()
  unlistenContent?.()
  unlistenAddon?.()
  for (const unlisten of addonUnlisteners) unlisten()
})

async function playInstance(instanceId: string) {
  await instances.ensureLoaded()
  if (!instances.instances.some(i => i.id === instanceId)) return
  await router.push(`/instance/${instanceId}`)
  mc.launch(instanceId).catch(() => {  })
}

onMounted(async () => {
  // Returns immediately when there is nothing to install; when there is, the
  // launcher restarts and none of this runs.
  await bootGate()

  activity.attach()
  if (!isContentWindow.value) {
    activity.withTask(t('activity.optimizing'), () => invoke('migrate_shared_dirs')).catch(() => {})
  }
  instances.ensureLoaded()
  telemetry.init()
  if (!isContentWindow.value) {
    accountRefreshTimer = setInterval(() => {
      if (spectra.user.value?.emailVerified === false) refreshAccount()
    }, 60_000)
    window.addEventListener('focus', refreshAccount)
    await cloud.load().catch(() => {})
    cloudTimer = setInterval(() => { void cloud.tick().catch(() => {}) }, 180_000)
    window.addEventListener('focus', refreshCloud)
  }
  spectra.refresh().then(() => {
    if (!isContentWindow.value) void cloud.tick().catch(() => {})
    if (!spectra.isSignedIn.value) return
    spectraNotifications.start()
    spectra.linkMinecraft()
  })
})
onBeforeUnmount(() => {
  activity.detach()
  spectraNotifications.stop()
  if (accountRefreshTimer) clearInterval(accountRefreshTimer)
  if (cloudTimer) clearInterval(cloudTimer)
  window.removeEventListener('focus', refreshAccount)
  window.removeEventListener('focus', refreshCloud)
})

function refreshCloud() {
  void cloud.tick().catch(() => {})
}
</script>
