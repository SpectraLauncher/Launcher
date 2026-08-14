<template>
  <UApp class="overflow-hidden">
    <!-- Titlebar -->
    <!-- On macOS the window controls live on the left and the launcher name is
         centered; everywhere else the name is on the left and controls right. -->
    <div data-tauri-drag-region class="z-10 relative flex w-full justify-between items-center h-10 px-2 text-gray-100 select-none">
      <template v-if="isMac">
        <div class="flex items-center">
          <WindowControls />
        </div>
        <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-2">
          <img src="/logo-transparent.png" alt="Spectra Launcher Icon" class="h-5 object-contain" />
          <span>Spectra Launcher</span>
        </div>
        <div class="flex items-center gap-3">
          <TitlebarActivity />
          <AccountButton />
        </div>
      </template>
      <template v-else>
        <div class="flex items-center gap-2 pl-2">
          <img src="/logo-transparent.png" alt="Spectra Launcher Icon" class="h-5 object-contain" />
          <span>Spectra Launcher</span>
        </div>
        <div class="flex items-center gap-3">
          <TitlebarActivity />
          <AccountButton />
          <WindowControls />
        </div>
      </template>
    </div>


    <NuxtLoadingIndicator color="aqua" errorColor="red" />
    <div :class="['relative w-screen h-[calc(100vh-2.5rem)] overflow-hidden text-[#eef1f5]', theme.bgClass]">


      <!-- Background texture + animated glows (from the Spectra design) -->
      <div
        class="pointer-events-none absolute inset-0"
        style="background-image:radial-gradient(rgba(255,255,255,0.035) 1px,transparent 1px);background-size:26px 26px;"
      />

      <div class="relative z-[1] h-full">
        <NuxtLayout>
          <NuxtPage />
        </NuxtLayout>

        <!-- Right-hand account panel. Collapsed it renders nothing, so the
             pages underneath keep the full width. -->
        <AccountSidebar />
      </div>
    </div>

    <LiveLogsModal />
    <CrashReportModal />
  </UApp>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

// Theme is applied in plugins/theme.client.ts; here we just expose the reactive
// background class to the shell.
const theme = useThemeStore()

// Drives the platform-specific titlebar layout (macOS puts controls on the left).
const { platform } = usePlatform()
const isMac = computed(() => platform.value === 'macos')

// Attach the global launch/install event hub once, so the titlebar activity
// indicator works regardless of which page is open.
const activity = useActivityCenter()
const instances = useInstancesStore()
const updater = useAutoUpdate()
const telemetry = useTelemetry()
const createModal = useCreateInstanceModal()
const spectra = useSpectraAccount()
const spectraNotifications = useSpectraNotifications()

// `spectra://share/<code>` links. The backend both emits the code (launcher
// already running) and parks it (link launched the app before the UI existed).
let unlistenShare: UnlistenFn | null = null
let unlistenAccount: UnlistenFn | null = null
onMounted(async () => {
  unlistenShare = await listen<string>('share://open', async (e) => {
    // Drain the parked copy of this same code so it can't reopen on next start.
    await invoke('take_pending_share').catch(() => {})
    createModal.openWithCode(e.payload)
  })
  const pending = await invoke<string | null>('take_pending_share')
  if (pending) createModal.openWithCode(pending)

  // Signing in happens in the browser and comes back as `spectra://auth/...`;
  // the backend swaps it for a session and tells us it is done.
  unlistenAccount = await listen('spectra://account', async () => {
    await spectra.refresh()
    spectraNotifications.start()
  })
})
onBeforeUnmount(() => {
  unlistenShare?.()
  unlistenAccount?.()
})

onMounted(() => {
  activity.attach()
  // Needed so the indicator can resolve instance names.
  instances.ensureLoaded()
  // Quietly look for a new release; surfaces an "Update" button in Settings.
  updater.checkForUpdates(true)
  // Anonymous usage stats (no-op unless opted in via Settings → Privacy).
  telemetry.init()
  // Spectra account (friends, shared instances) — starts polling once there is
  // a session; a signed-out launcher does nothing here.
  spectra.refresh().then(() => {
    if (spectra.isSignedIn.value) spectraNotifications.start()
  })
})
onBeforeUnmount(() => {
  activity.detach()
  spectraNotifications.stop()
})
</script>
