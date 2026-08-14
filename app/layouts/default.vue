<template>
  <div class="app h-full w-full flex">


      <div class="flex flex-col w-64 px-4 h-full py-4 backdrop-blur-lg">

        <SelectedInstanceCard />

        <div id="menu" class="flex flex-col gap-2">
          <NuxtLink
            v-for="item in menu"
            :key="item.to"
            :to="item.to"
            class="flex items-center justify-start gap-2 relative py-1 px-3 rounded-lg overflow-hidden duration-300 hover:bg-primary-500/5"
            exact-active-class="bg-primary-500/10"
            v-slot="{ isExactActive }"
          >
            <div :class="['absolute left-0 top-1/2 -translate-y-1/2 w-1 rounded-md bg-primary-500 duration-300', isExactActive ? 'opacity-100 h-4' : ' h-0']"></div>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" v-html="item.icon"></svg>
            <p>{{ $t(item.label) }}</p>
          </NuxtLink>
        </div>

        <div class="flex flex-1 min-h-0 flex-col gap-2 py-4 mt-4 border-t border-gray-800">
          <!-- instances list -->
          <div class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-1">
            <!-- skeleton rows during the first load -->
            <template v-if="instances.loading && !instances.loaded">
              <div v-for="n in 6" :key="`inst-sk-${n}`" class="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5">
                <div class="size-7 shrink-0 animate-pulse rounded-md bg-white/5" />
                <div class="min-w-0 flex-1 space-y-1.5">
                  <div class="h-3 w-3/4 animate-pulse rounded bg-white/5" />
                  <div class="h-2 w-2/5 animate-pulse rounded bg-white/5" />
                </div>
              </div>
            </template>
            <SidebarInstanceItem
              v-for="instance in instances.instances"
              :key="instance.id"
              :instance="instance"
            />
          </div>

          <button class="w-full py-1 px-3 duration-300 hover:bg-primary-500/5 flex justify-start items-center gap-2 rounded-lg" @click="openCreate()">
            + {{ $t('nav.newInstance') }}
          </button>
        </div>


      </div>
      <div class="flex-1 w-full rounded-tl-xl border-t border-l border-[#13161d]">
        <slot />
      </div>

    <CreateInstanceModal />
    <ModrinthBrowser />
    <ExportInstanceModal />
    <ExportModListModal />
    <LinkModsModal />
    <BlockedModsModal />
    <ChangeLoaderModal />
    <OnboardingModal />

    <!-- drag & drop overlay -->
    <Transition name="fade">
      <div v-if="dragging" class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div class="flex flex-col items-center gap-3 rounded-2xl border-2 border-dashed border-primary-500/60 bg-primary-500/10 px-12 py-10">
          <UIcon name="i-lucide-download" class="size-10 text-primary-400" />
          <p class="text-lg font-semibold">{{ $t('drop.title') }}</p>
          <p class="text-sm text-muted">{{ onInstance ? $t('drop.onInstance') : $t('drop.onHome') }}</p>
        </div>
      </div>
    </Transition>
	</div>
</template>

<style scoped>
.fade-enter-active, .fade-leave-active { transition: opacity 0.15s ease; }
.fade-enter-from, .fade-leave-to { opacity: 0; }
</style>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Instance } from '~/types/launcher'

const instances = useInstancesStore()
const accounts = useAccountStore()
const router = useRouter()
const route = useRoute()
const toast = useToast()
const { t } = useI18n()
const { open: openCreate } = useCreateInstanceModal()

onMounted(() => {
  instances.ensureLoaded()
  accounts.load()
})

// --- drag & drop ---
const dragging = ref(false)
// The instance currently being viewed (drops add content to it).
const currentInstanceId = computed(() => (route.path.startsWith('/instance/') ? String(route.params.id) : null))
const onInstance = computed(() => currentInstanceId.value !== null)

let unlistenDrop: UnlistenFn | null = null
onMounted(async () => {
  unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
    const p = event.payload
    if (p.type === 'enter' || p.type === 'over') {
      dragging.value = true
    } else if (p.type === 'leave') {
      dragging.value = false
    } else if (p.type === 'drop') {
      dragging.value = false
      await handleDrop(p.paths)
    }
  })
})
onBeforeUnmount(() => unlistenDrop?.())

async function handleDrop(paths: string[]) {
  const wanted = paths.filter(p => /\.(mrpack|zip|jar)$/i.test(p))
  if (!wanted.length) return
  try {
    const res = await invoke<{ instances: Instance[], added: number, skipped: number }>('import_dropped', {
      instanceId: currentInstanceId.value,
      paths: wanted,
    })
    if (res.instances.length) {
      await instances.load()
      toast.add({ title: t('drop.imported', { n: res.instances.length }), color: 'success' })
      const first = res.instances[0]
      if (first) router.push(`/instance/${first.id}`)
    }
    if (res.added) {
      toast.add({ title: t('drop.added', { n: res.added }), color: 'success' })
    }
    if (!res.instances.length && !res.added) {
      toast.add({ title: t('drop.nothing'), color: 'neutral' })
    }
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

// Skins only apply to Microsoft accounts (Mojang skin API needs a real session).
const isMicrosoft = computed(() => accounts.activeAccount?.kind === 'microsoft')

interface MenuItem {
  /** Route path. */
  to: string
  /** i18n key for the label. */
  label: string
  /** Inner SVG markup for the 24x24 icon (kept inline so icons work offline). */
  icon: string
  /** Only show this item when the active account is a Microsoft account. */
  microsoftOnly?: boolean
}

// Sidebar menu. Add an entry here to add a sidebar item — no markup to repeat.
const allMenu: MenuItem[] = [
  {
    to: '/',
    label: 'nav.home',
    icon: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  },
  {
    to: '/worlds',
    label: 'nav.worlds',
    icon: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18 14 14 0 0 1 0-18z"/>',
  },
  {
    to: '/screenshots',
    label: 'nav.screenshots',
    icon: '<rect x="3" y="3" width="18" height="18" rx="2.5"/><circle cx="8.5" cy="8.5" r="1.6"/><path d="M21 15l-5-5L5 21"/>',
  },
  {
    to: '/skins',
    label: 'nav.skins',
    icon: '<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>',
    microsoftOnly: true,
  },
  {
    to: '/settings',
    label: 'nav.settings',
    icon: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
  },
]

const menu = computed(() => allMenu.filter(i => !i.microsoftOnly || isMicrosoft.value))
</script>
