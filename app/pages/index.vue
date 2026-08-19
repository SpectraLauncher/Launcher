<template>
  <UContextMenu :items="contextItems">
    <div class="h-full overflow-y-auto p-8" @contextmenu.capture="onContext">
      <!-- header -->
      <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ $t('library.title') }}</h1>
          <p class="mt-1 text-sm text-muted">{{ $t('library.count', { n: instances.instances.length }) }}</p>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <UInput v-model="search" icon="i-lucide-search" variant="soft" :placeholder="$t('library.search')" class="w-56" />
          <USelect v-model="loaderFilter" variant="soft" :items="loaderFilterItems" class="w-40" />
          <UButton icon="i-lucide-plus" :label="$t('nav.newInstance')" @click="openCreate()" />
        </div>
      </div>

      <!-- sponsored section (always on top; hidden when SPONSORS is empty or user dismissed) -->
      <section v-if="sponsor.visible.value && !filtering" class="mb-8">
        <div class="mb-3.5 flex items-center gap-2">
          <span class="flex items-center gap-1.5 text-sm font-semibold text-amber-400">
            <UIcon name="i-lucide-sparkles" class="size-4" />
            {{ $t('library.sponsored') }}
          </span>
          <div class="h-px flex-1 bg-amber-400/15" />
        </div>
        <div class="flex flex-col gap-2">
          <button
            v-for="s in sponsor.sponsors"
            :key="s.id"
            type="button"
            class="group flex w-full items-center gap-4 rounded-2xl border border-amber-400/25 bg-linear-[120deg] from-amber-500/12 to-transparent p-4 text-left transition hover:border-amber-400/50 hover:from-amber-500/18"
            @click="onSponsorClick(s)"
          >
            <img :src="s.iconUrl" :alt="s.title" class="size-14 shrink-0 rounded-xl object-cover shadow-[0_4px_14px_rgba(0,0,0,0.35)]" />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="truncate font-semibold">{{ s.title }}</span>
                <UBadge color="warning" variant="subtle" size="xs" :label="$t('library.sponsoredBadge')" />
                <!-- type badge -->
                <UBadge
                  v-if="s.type !== 'modpack'"
                  color="neutral"
                  variant="subtle"
                  size="xs"
                  :label="s.type === 'server' ? $t('library.sponsorServer') : $t('library.sponsorHosting')"
                />
              </div>
              <p class="mt-1 line-clamp-2 text-xs text-muted">{{ s.description }}</p>
              <!-- server address -->
              <p v-if="s.type === 'server' && (s as any).address" class="mt-0.5 font-mono text-xs text-amber-300/70">{{ (s as any).address }}</p>
            </div>
            <span class="hidden shrink-0 items-center gap-1.5 rounded-lg bg-amber-500/15 px-3 py-2 text-sm font-medium text-amber-300 transition group-hover:bg-amber-500/25 sm:flex">
              <UIcon :name="s.type === 'modpack' ? 'i-lucide-download' : 'i-lucide-external-link'" class="size-4" />
              {{ s.type === 'modpack' ? $t('library.install') : $t('library.sponsorVisit') }}
            </span>
          </button>
        </div>
      </section>

      <!-- skeleton grid during the first load -->
      <div
        v-if="instances.loading && !instances.loaded"
        class="grid gap-3.5"
        style="grid-template-columns:repeat(auto-fill,minmax(248px,1fr))"
      >
        <div
          v-for="n in 8"
          :key="`card-sk-${n}`"
          class="rounded-2xl border border-default bg-white/3 p-4"
        >
          <div class="flex items-center gap-3">
            <div class="size-13 shrink-0 animate-pulse rounded-[13px] bg-white/5" />
            <div class="min-w-0 flex-1 space-y-2">
              <div class="h-4 w-3/4 animate-pulse rounded bg-white/5" />
              <div class="h-3 w-2/5 animate-pulse rounded bg-white/5" />
            </div>
          </div>
        </div>
      </div>

      <!-- empty library -->
      <div v-else-if="!instances.instances.length" class="flex flex-col items-center justify-center gap-4 py-24 text-center">
        <UIcon name="i-lucide-box" class="size-12 text-neutral-600" />
        <p class="text-sm text-muted">{{ $t('library.empty') }}</p>
        <UButton icon="i-lucide-plus" :label="$t('nav.newInstance')" @click="openCreate()" />
      </div>

      <!-- no results -->
      <div v-else-if="filtering && totalVisible === 0" class="py-24 text-center text-sm text-muted">
        {{ $t('library.noResults') }}
      </div>

      <!-- groups (the whole list is draggable to reorder groups) -->
      <VueDraggable
        v-else
        v-model="layout.groups.value"
        :animation="160"
        :disabled="filtering"
        :force-fallback="true"
        :fallback-on-body="true"
        handle=".mk-group-handle"
        ghost-class="mk-group-ghost"
        class="space-y-8"
        @end="layout.onGroupsReordered()"
      >
        <section v-for="group in layout.groups.value" v-show="!filtering || visibleCount(group) > 0" :key="group.id">
          <div class="group/h mb-3.5 flex items-center gap-2">
            <!-- collapse toggle -->
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-md py-0.5 text-sm font-semibold text-neutral-300 transition hover:text-white"
              @click="layout.toggleCollapse(group.id)"
            >
              <UIcon
                :name="group.collapsed ? 'i-lucide-chevron-right' : 'i-lucide-chevron-down'"
                class="size-4 text-neutral-500"
              />
              {{ group.name ?? $t('library.ungrouped') }}
            </button>
            <!-- drag handle (named groups only) -->
            <UIcon
              v-if="group.name && !filtering"
              name="i-lucide-grip-vertical"
              class="mk-group-handle size-4 cursor-grab text-neutral-600 opacity-0 transition hover:text-neutral-300 group-hover/h:opacity-100 active:cursor-grabbing"
              :title="$t('library.dragGroup')"
            />
            <UButton
              v-if="group.name"
              icon="i-lucide-trash-2"
              size="xs"
              color="neutral"
              variant="ghost"
              class="opacity-0 transition group-hover/h:opacity-100"
              :title="$t('library.deleteGroup')"
              @click="layout.removeGroup(group.id)"
            />
            <div class="h-px flex-1 bg-white/6" />
                        <span class="text-xs text-neutral-500">{{ filtering ? visibleCount(group) : group.items.length }}</span>

          </div>

          <VueDraggable
            v-show="!group.collapsed"
            v-model="group.items"
            group="library"
            :animation="160"
            :disabled="filtering"
            :force-fallback="true"
            :fallback-on-body="true"
            :fallback-tolerance="6"
            ghost-class="mk-card-ghost"
            class="grid min-h-16 gap-3.5"
            style="grid-template-columns:repeat(auto-fill,minmax(248px,1fr))"
            @end="onInstanceDragEnd"
            @start="onInstanceDragStart"
          >
            <div
              v-for="item in group.items"
              v-show="matches(item)"
              :key="item.id"
              :data-instance-id="item.id"
              class="mk-card group cursor-grab select-none rounded-2xl border border-default bg-white/3 backdrop-blur-xs p-4 transition-colors hover:border-primary-500/40 hover:bg-white/5 active:cursor-grabbing"
              @click="enter(item.id)"
            >
              <div class="flex items-center gap-3">
                <!-- hover the card, launch from the icon -->
                <div class="relative size-13 shrink-0">
                  <InstanceIcon
                    :instance="item"
                    class="size-full rounded-[13px] text-[22px] shadow-[0_4px_14px_rgba(0,0,0,0.35)] transition duration-150 group-hover:brightness-[0.35]"
                  />
                  <button
                    type="button"
                    class="absolute inset-0 flex cursor-pointer items-center justify-center rounded-[13px] opacity-0 transition duration-150 group-hover:opacity-100 focus-visible:opacity-100"
                    :aria-label="$t('ctx.play')"
                    :title="$t('ctx.play')"
                    @click.stop="quickPlay(item)"
                  >
                    <UIcon
                      :name="busy(item.id) ? 'i-lucide-loader-circle' : 'i-lucide-play'"
                      class="size-6 text-primary-400 drop-shadow-[0_2px_6px_rgba(0,0,0,0.6)]"
                      :class="{ 'animate-spin': busy(item.id) }"
                    />
                  </button>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="truncate font-semibold">{{ item.name }}</div>
                  <div class="mt-1.5 flex flex-wrap items-center gap-2">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"><!-- Icon from Lucide by Lucide Contributors - https://github.com/lucide-icons/lucide/blob/main/LICENSE --><path fill="none" stroke="#888888" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 11h4M8 9v4m7-1h.01M18 10h.01m-.69-5H6.68a4 4 0 0 0-3.978 3.59l-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258q-.01-.075-.017-.151A4 4 0 0 0 17.32 5"/></svg>
                    <span class="font-mono text-[11px] text-neutral-400">{{ item.mc_version }}</span>
                    <span class="font-mono text-[11px] text-neutral-400">{{ loaderLabel(item.loader.type) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </VueDraggable>
        </section>
      </VueDraggable>
    </div>
  </UContextMenu>

  <!-- create group modal -->
  <UModal v-model:open="createGroupOpen" :title="$t('library.createGroupTitle')">
    <template #body>
      <UFormField :label="$t('library.groupName')">
        <UInput
          v-model="newGroupName"
          :placeholder="$t('library.groupNamePlaceholder')"
          autofocus
          class="w-full"
          @keydown.enter="confirmCreateGroup"
        />
      </UFormField>
    </template>
    <template #footer>
      <div class="flex w-full items-center gap-3">
        <ModalHint>{{ $t('library.groupHint') }}</ModalHint>
        <div class="ml-auto flex shrink-0 gap-2">
          <UButton variant="ghost" color="neutral" :label="$t('common.cancel')" @click="createGroupOpen = false" />
          <UButton :label="$t('common.add')" :disabled="!newGroupName.trim()" @click="confirmCreateGroup" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { VueDraggable } from 'vue-draggable-plus'
import type { Instance, LoaderType } from '~/types/launcher'
import type { DisplayGroup } from '~/composables/useLibraryLayout'

const instances = useInstancesStore()
const layout = useLibraryLayout()
const mc = useMinecraftLaunch()
const activity = useActivityCenter()
const router = useRouter()
const toast = useToast()
const { t } = useI18n()
const { open: openCreate } = useCreateInstanceModal()
const sponsor = useSponsor()
const browser = useModrinthBrowser()

// Opens the appropriate action when a sponsor card is clicked.
function onSponsorClick(s: ReturnType<typeof useSponsor>['sponsors'][number]) {
  if (s.type === 'modpack' && s.modrinthSlug) {
    browser.open({
      kind: 'modpack',
      mode: 'createModpack',
      query: s.modrinthSlug,
      onInstalled: (instance) => {
        instances.load()
        if (instance) router.push(`/instance/${instance.id}`)
      },
    })
  } else {
    // For servers, hosting, and modpacks without a Modrinth slug — open in browser.
    openExternal(s.url).catch(() => {})
  }
}

onMounted(async () => {
  await instances.ensureLoaded()
  layout.reconcile(instances.instances)
})

// Re-sync the layout when instances are added/removed.
watch(
  () => instances.instances.map(i => i.id).join(','),
  () => layout.reconcile(instances.instances),
)

// --- search / filter ---
const search = ref('')
const loaderFilter = ref<'all' | LoaderType>('all')
const loaderFilterItems = computed(() => [
  { label: t('library.allLoaders'), value: 'all' },
  { label: 'Vanilla', value: 'vanilla' },
  { label: 'Fabric', value: 'fabric' },
  { label: 'NeoForge', value: 'neoforge' },
  { label: 'Forge', value: 'forge' },
  { label: 'Quilt', value: 'quilt' },
])

const filtering = computed(() => !!search.value.trim() || loaderFilter.value !== 'all')

function matches(item: Instance): boolean {
  if (loaderFilter.value !== 'all' && item.loader.type !== loaderFilter.value) return false
  if (search.value && !item.name.toLowerCase().includes(search.value.toLowerCase())) return false
  return true
}
const visibleCount = (group: DisplayGroup) => group.items.filter(matches).length
const totalVisible = computed(() => layout.groups.value.reduce((n, g) => n + visibleCount(g), 0))

// --- navigation / actions ---
// A drag releases on the same card it grabbed, which fires a `click`. Guard
// against that so dragging an instance never accidentally opens it. The card
// only becomes a drag past `fallback-tolerance` pixels, so the shaky hand that
// moves 1-2px while clicking still opens the instance.
let suppressClickUntil = 0
const dragJustEnded = () => Date.now() < suppressClickUntil

const enter = (id: string) => {
  if (dragJustEnded()) return
  router.push(`/instance/${id}`)
}

/** Installing or already running — a second launch would be refused anyway. */
const busy = (id: string) => activity.list.value.some(a => a.instanceId === id)

/** Launches straight from the library; progress shows in the titlebar. */
const quickPlay = (item: Instance) => {
  if (dragJustEnded() || busy(item.id)) return
  mc.launch(item.id).catch(() => { /* surfaced by the activity centre */ })
}

const onInstanceDragStart = () => {
  suppressClickUntil = Number.POSITIVE_INFINITY
}
const onInstanceDragEnd = () => {
  layout.persist()
  // Keep the guard alive briefly so the trailing click is swallowed.
  suppressClickUntil = Date.now() + 150
}

async function play(item: Instance) {
  await router.push(`/instance/${item.id}`)
  mc.launch(item.id).catch(() => {})
}

/** Drops a desktop shortcut that opens `spectra://launch/<id>`. */
async function createShortcut(item: Instance) {
  try {
    await invoke('create_desktop_shortcut', { id: item.id })
    toast.add({ title: t('instance.shortcutCreated'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function duplicate(item: Instance) {
  try {
    await invoke('duplicate_instance', { id: item.id })
    await instances.load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function openFolder(item: Instance) {
  try {
    await invoke('open_instance_folder', { id: item.id })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function copyPath(item: Instance) {
  try {
    const path = await invoke<string>('get_instance_path', { id: item.id })
    await navigator.clipboard.writeText(path)
    toast.add({ title: t('library.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

// --- context menu (dynamic: instance vs empty space) ---
const contextTarget = ref<Instance | null>(null)

function onContext(e: MouseEvent) {
  const el = (e.target as HTMLElement | null)?.closest('[data-instance-id]') as HTMLElement | null
  const id = el?.dataset.instanceId
  contextTarget.value = id ? instances.instances.find(i => i.id === id) ?? null : null
}

const emptyMenu = computed(() => [[
  { label: t('library.createGroup'), icon: 'i-lucide-folder-plus', onSelect: openCreateGroup },
  { label: t('nav.newInstance'), icon: 'i-lucide-plus', onSelect: () => openCreate() },
]])

function instanceMenu(item: Instance) {
  return [
    [
      { label: t('ctx.play'), icon: 'i-lucide-play', onSelect: () => play(item) },
      { label: t('ctx.view'), icon: 'i-lucide-eye', onSelect: () => enter(item.id) },
      { label: t('ctx.duplicate'), icon: 'i-lucide-copy', onSelect: () => duplicate(item) },
    ],
    [
      { label: t('instance.createShortcut'), icon: 'i-lucide-app-window', onSelect: () => createShortcut(item) },
      { label: t('ctx.openFolder'), icon: 'i-lucide-folder', onSelect: () => openFolder(item) },
      { label: t('ctx.copyPath'), icon: 'i-lucide-clipboard', onSelect: () => copyPath(item) },
    ],
    [
      { label: t('common.remove'), icon: 'i-lucide-trash-2', color: 'error' as const, onSelect: () => instances.remove(item.id) },
    ],
  ]
}

const contextItems = computed(() =>
  contextTarget.value ? instanceMenu(contextTarget.value) : emptyMenu.value,
)

// --- create group ---
const createGroupOpen = ref(false)
const newGroupName = ref('')

function openCreateGroup() {
  newGroupName.value = ''
  createGroupOpen.value = true
}
function confirmCreateGroup() {
  if (!newGroupName.value.trim()) return
  layout.createGroup(newGroupName.value)
  createGroupOpen.value = false
}
</script>

<style>
/* Placeholder shown at the drop target while dragging. */
.mk-card-ghost {
  opacity: 0.4;
  border-style: dashed;
  border-color: var(--ui-primary);
  background: color-mix(in oklab, var(--ui-primary) 8%, transparent);
}
.mk-group-ghost {
  opacity: 0.5;
}

/* The clone that follows the cursor (forceFallback). It is appended to <body>,
   so it has no dark app background behind it — give it a solid, opaque surface
   so the card underneath doesn't bleed through. */
.sortable-fallback {
  opacity: 1 !important;
  background: var(--ui-bg-elevated, #1c1c1f) !important;
  backdrop-filter: none !important;
  border-color: var(--ui-primary) !important;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.55);
}
</style>
