<template>
  <UApp class="overflow-hidden">
    <!-- Titlebar -->
    <!-- On macOS the window controls live on the left and the launcher name is
         centered; everywhere else the name is on the left and controls right. -->
    <!-- Teleported to <body> on purpose: `#__nuxt` carries `isolation: isolate`,
         so anything inside it is painted below the modals — which portal to <body>
         — whatever its z-index. The window chrome has to leave that stacking
         context to stay grabbable. Out here `z-[100]` beats the overlays (Nuxt UI
         gives them no z-index at all) and `pointer-events-auto` survives the
         `pointer-events: none` an open dialog puts on <body>, so the mousedown
         Tauri needs for `data-tauri-drag-region` still happens. `pointerdown.self.stop`
         keeps that press away from the dialog's document-level outside-press
         listener, so grabbing the bar moves the window instead of closing the modal. -->
    <Teleport to="body">
      <div
        data-tauri-drag-region
        class="pointer-events-auto fixed inset-x-0 top-0 z-[100] flex justify-between items-center h-10 px-2 text-gray-100 select-none"
        @pointerdown.self.stop
      >
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

      <!-- rides along so its z-index:999999 lands in the same stacking context and
           the route-change line stays visible over the bar -->
      <NuxtLoadingIndicator color="aqua" errorColor="red" />
    </Teleport>

    <!-- the bar is fixed now, so this holds the 2.5rem it used to take up -->
    <div class="h-10" />

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
const router = useRouter()
const mc = useMinecraftLaunch()
const updater = useAutoUpdate()
const telemetry = useTelemetry()
const createModal = useCreateInstanceModal()
const spectra = useSpectraAccount()
const spectraNotifications = useSpectraNotifications()

// `spectra://share/<code>` links. The backend both emits the code (launcher
// already running) and parks it (link launched the app before the UI existed).
let unlistenShare: UnlistenFn | null = null
let unlistenAccount: UnlistenFn | null = null
let unlistenLaunch: UnlistenFn | null = null
onMounted(async () => {
  unlistenShare = await listen<string>('share://open', async (e) => {
    // Drain the parked copy of this same code so it can't reopen on next start.
    await invoke('take_pending_share').catch(() => {})
    createModal.openWithCode(e.payload)
  })
  const pending = await invoke<string | null>('take_pending_share')
  if (pending) createModal.openWithCode(pending)

  // Desktop shortcuts: `spectra://launch/<id>`. Usually the click is what
  // started the launcher, so the parked copy is the one that fires.
  unlistenLaunch = await listen<string>('launch://open', async (e) => {
    await invoke('take_pending_launch').catch(() => {})
    playInstance(e.payload)
  })
  const pendingLaunch = await invoke<string | null>('take_pending_launch')
  if (pendingLaunch) playInstance(pendingLaunch)

  // Signing in happens in the browser and comes back as `spectra://auth/...`;
  // the backend swaps it for a session and tells us it is done.
  unlistenAccount = await listen('spectra://account', async () => {
    await spectra.refresh()
    spectraNotifications.start()
    spectra.linkMinecraft()
  })
})
onBeforeUnmount(() => {
  unlistenShare?.()
  unlistenAccount?.()
  unlistenLaunch?.()
})

/** Opens the instance page and starts the game, for shortcut deep links. */
async function playInstance(instanceId: string) {
  await instances.ensureLoaded()
  if (!instances.instances.some(i => i.id === instanceId)) return
  await router.push(`/instance/${instanceId}`)
  mc.launch(instanceId).catch(() => { /* surfaced on the instance page */ })
}

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
    if (!spectra.isSignedIn.value) return
    spectraNotifications.start()
    // Names change, so this runs every start rather than only on sign-in.
    spectra.linkMinecraft()
  })
})
onBeforeUnmount(() => {
  activity.detach()
  spectraNotifications.stop()
})
</script>
