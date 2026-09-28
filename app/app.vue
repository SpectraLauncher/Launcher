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
      <LiveLogsModal />
      <CrashReportModal />
      <StartupUpdate />
    </template>
  </UApp>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

const { t } = useI18n()
const theme = useThemeStore()

const route = useRoute()
const isContentWindow = computed(() => route.path.startsWith('/browser'))
const windowTitle = computed(() => (isContentWindow.value ? 'Spectra — content' : 'Spectra Launcher'))

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

async function openAccountSettings() {
  const url = await invoke<string>('spectra_account_settings_url')
  await openExternal(url)
}

function refreshAccount() {
  if (spectra.isSignedIn.value) spectra.refresh()
}

let accountRefreshTimer: ReturnType<typeof setInterval> | null = null

const contentWindow = useContentWindow()

let unlistenContent: UnlistenFn | null = null
let unlistenShare: UnlistenFn | null = null
let unlistenAccount: UnlistenFn | null = null
let unlistenLaunch: UnlistenFn | null = null
onMounted(async () => {
  await bootGate()

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

  unlistenAccount = await listen('spectra://account', async () => {
    await spectra.refresh()
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
  unlistenContent?.()
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
  }
  spectra.refresh().then(() => {
    if (!spectra.isSignedIn.value) return
    spectraNotifications.start()
    spectra.linkMinecraft()
  })
})
onBeforeUnmount(() => {
  activity.detach()
  spectraNotifications.stop()
  if (accountRefreshTimer) clearInterval(accountRefreshTimer)
  window.removeEventListener('focus', refreshAccount)
})
</script>
