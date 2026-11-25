<template>
  <UContextMenu :items="contextItems">
    <div class="h-full overflow-y-auto" @contextmenu.capture="onContext">
      <div class="mx-auto max-w-[1400px] px-8 pb-12 pt-7">
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div class="flex items-baseline gap-3">
            <h1 class="text-[22px] font-semibold tracking-[-0.01em] text-highlighted">{{ $t('library.title') }}</h1>
            <span class="text-sm tabular-nums text-muted">{{ $t('library.count', { n: instances.instances.length }) }}</span>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <AddonSlot name="home.header" />
            <UButton icon="i-lucide-plus" color="neutral" variant="soft" :label="$t('nav.newInstance')" @click="openCreate()" />
          </div>
        </div>

        <div v-if="instances.instances.length" class="mt-5 flex flex-wrap items-center gap-1.5" role="radiogroup" :aria-label="$t('library.allLoaders')">
          <button
            v-for="opt in loaderFilterItems"
            :key="opt.value"
            type="button"
            role="radio"
            :aria-checked="loaderFilter === opt.value"
            class="h-8 rounded-full px-3.5 text-[13px] font-medium transition-colors"
            :class="loaderFilter === opt.value
 ? 'bg-primary text-inverted'
 : 'bg-elevated text-toned hover:bg-accented hover:text-highlighted'"
            @click="loaderFilter = opt.value as typeof loaderFilter"
          >
            {{ opt.label }}
          </button>
        </div>

        <section v-if="!filtering && continueInstance" class="mt-8">
          <h2 class="mb-3 text-[15px] font-semibold text-highlighted">{{ $t('instanceCard.continue') }}</h2>
          <SelectedInstanceCard />
        </section>

        <section v-if="sponsor.visible.value && !filtering" class="mt-8">
          <h2 class="mb-3 text-[15px] font-semibold text-highlighted">{{ $t('library.sponsored') }}</h2>
          <div class="flex flex-col gap-2">
            <button
              v-for="s in sponsor.sponsors"
              :key="s.id"
              type="button"
              class="group flex w-full items-center gap-4 rounded-xl border border-default bg-muted p-3.5 text-left transition-colors hover:border-(--ui-border-accented) hover:bg-elevated"
              @click="onSponsorClick(s)"
            >
              <img :src="s.iconUrl" :alt="s.title" class="size-12 shrink-0 rounded-lg object-cover" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                  <span class="truncate font-semibold text-highlighted">{{ s.title }}</span>
                  <UBadge color="warning" variant="subtle" size="xs" :label="$t('library.sponsoredBadge')" />
                  <UBadge
                    v-if="s.type !== 'modpack'"
                    color="neutral"
                    variant="subtle"
                    size="xs"
                    :label="s.type === 'server' ? $t('library.sponsorServer') : $t('library.sponsorHosting')"
                  />
                </div>
                <p class="mt-0.5 line-clamp-2 text-[13px] text-muted">{{ s.description }}</p>
                <p v-if="s.type === 'server' && (s as any).address" class="mt-0.5 font-mono text-xs text-dimmed">{{ (s as any).address }}</p>
              </div>
              <span class="hidden shrink-0 items-center gap-1.5 rounded-lg bg-accented px-3 py-1.5 text-[13px] font-medium text-highlighted transition-colors group-hover:bg-primary group-hover:text-inverted sm:flex">
                <UIcon :name="s.type === 'modpack' ? 'i-lucide-download' : 'i-lucide-external-link'" class="size-4" />
                {{ s.type === 'modpack' ? $t('library.install') : $t('library.sponsorVisit') }}
              </span>
            </button>
          </div>
        </section>

        <div
          v-if="instances.loading && !instances.loaded"
          class="mt-8 grid gap-3"
          style="grid-template-columns:repeat(auto-fill,minmax(260px,1fr))"
        >
          <div
            v-for="n in 8"
            :key="`card-sk-${n}`"
            class="flex items-center gap-3 rounded-xl border border-default bg-muted p-3"
          >
            <div class="size-12 shrink-0 animate-pulse rounded-lg bg-elevated" />
            <div class="min-w-0 flex-1 space-y-2">
              <div class="h-3.5 w-3/4 animate-pulse rounded bg-elevated" />
              <div class="h-3 w-2/5 animate-pulse rounded bg-elevated" />
            </div>
          </div>
        </div>

        <div v-else-if="!instances.instances.length" class="mt-16 flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-(--ui-border-accented) px-6 py-20 text-center">
          <UIcon name="i-lucide-box" class="size-10 text-dimmed" />
          <p class="max-w-sm text-sm text-muted">{{ $t('library.empty') }}</p>
          <UButton icon="i-lucide-plus" :label="$t('nav.newInstance')" class="mt-2" @click="openCreate()" />
        </div>

        <div v-else-if="filtering && totalVisible === 0" class="mt-16 text-center text-sm text-muted">
          {{ $t('library.noResults') }}
        </div>

        <VueDraggable
          v-else
          v-model="layout.groups.value"
          :animation="160"
          :disabled="filtering"
          :force-fallback="true"
          :fallback-on-body="true"
          handle=".mk-group-handle"
          ghost-class="mk-group-ghost"
          class="mt-8 space-y-8"
          @end="layout.onGroupsReordered()"
        >
          <section v-for="group in layout.groups.value" v-show="!filtering || visibleCount(group) > 0" :key="group.id">
            <div class="group/h mb-3 flex items-center gap-2">
              <button
                type="button"
                class="flex items-center gap-1.5 rounded-md text-[15px] font-semibold text-highlighted transition-colors hover:text-primary"
                :aria-expanded="!group.collapsed"
                @click="layout.toggleCollapse(group.id)"
              >
                {{ group.name ?? $t('library.ungrouped') }}
                <UIcon
                  :name="group.collapsed ? 'i-lucide-chevron-right' : 'i-lucide-chevron-down'"
                  class="size-4 text-dimmed"
                />
              </button>
              <span class="text-[13px] tabular-nums text-dimmed">{{ filtering ? visibleCount(group) : group.items.length }}</span>
              <UIcon
                v-if="group.name && !filtering"
                name="i-lucide-grip-vertical"
                class="mk-group-handle size-4 cursor-grab text-dimmed opacity-0 transition hover:text-toned group-hover/h:opacity-100 active:cursor-grabbing"
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
                :aria-label="$t('library.deleteGroup')"
                @click="layout.removeGroup(group.id)"
              />
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
              class="grid min-h-16 gap-3"
              style="grid-template-columns:repeat(auto-fill,minmax(260px,1fr))"
              @end="onInstanceDragEnd"
              @start="onInstanceDragStart"
            >
              <div
                v-for="item in group.items"
                v-show="matches(item)"
                :key="item.id"
                :data-instance-id="item.id"
                class="mk-card group flex cursor-pointer select-none items-center gap-3 rounded-xl border border-default bg-muted p-3 transition-colors hover:border-(--ui-border-accented) hover:bg-elevated"
                @click="enter(item.id)"
              >
                <InstanceIcon :instance="item" class="size-12 rounded-lg text-[19px]" />
                <div class="min-w-0 flex-1">
                  <div class="truncate font-semibold text-highlighted">{{ item.name }}</div>
                  <div class="mt-0.5 truncate text-[13px] tabular-nums text-muted">
                    {{ loaderLabel(item.loader.type) }} {{ item.mc_version }}
                  </div>
                </div>
                <button
                  type="button"
                  class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary transition-[background-color,color,transform] group-hover:bg-primary group-hover:text-inverted focus-visible:bg-primary focus-visible:text-inverted active:scale-95 disabled:opacity-60"
                  :disabled="busy(item.id)"
                  :aria-label="`${$t('ctx.play')}: ${item.name}`"
                  :title="$t('ctx.play')"
                  @click.stop="quickPlay(item)"
                >
                  <UIcon v-if="busy(item.id)" name="i-lucide-loader-circle" class="size-4 animate-spin" />
                  <svg v-else class="size-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l13-7.5z" /></svg>
                </button>
              </div>
            </VueDraggable>
          </section>
        </VueDraggable>
      </div>
    </div>
  </UContextMenu>

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
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { VueDraggable } from 'vue-draggable-plus'
import type { Instance, LoaderType } from '~/types/launcher'
import type { DisplayGroup } from '~/composables/useLibraryLayout'

const instances = useInstancesStore()
const layout = useLibraryLayout()
const mc = useMinecraftLaunch()
const activity = useActivityCenter()
const router = useRouter()
const toast = useToast()
const { t, locale } = useI18n()
const { open: openCreate } = useCreateInstanceModal()
const sponsor = useSponsor()
const browser = useContentWindow()

function onSponsorClick(s: ReturnType<typeof useSponsor>['sponsors'][number]) {
  if (s.type === 'modpack' && s.modrinthSlug) {
    browser.open({
      kind: 'modpack',
      mode: 'createModpack',
      query: s.modrinthSlug,
    })
  } else {
    openExternal(s.url).catch(() => {})
  }
}

onMounted(async () => {
  await instances.ensureLoaded()
  layout.reconcile(instances.instances)
})

watch(
  () => instances.instances.map(i => i.id).join(','),
  () => layout.reconcile(instances.instances),
)

const search = useState('library-search', () => '')
const continueInstance = computed(() => instances.instances.some(i => i.last_played))
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

let suppressClickUntil = 0
const dragJustEnded = () => Date.now() < suppressClickUntil

const enter = (id: string) => {
  if (dragJustEnded()) return
  router.push(`/instance/${id}`)
}

const busy = (id: string) => activity.list.value.some(a => a.instanceId === id)

const quickPlay = (item: Instance) => {
  if (dragJustEnded() || busy(item.id)) return
  mc.launch(item.id).catch(() => {  })
}

const onInstanceDragStart = () => {
  suppressClickUntil = Number.POSITIVE_INFINITY
}
const onInstanceDragEnd = () => {
  layout.persist()
  suppressClickUntil = Date.now() + 150
}

async function play(item: Instance) {
  await router.push(`/instance/${item.id}`)
  mc.launch(item.id).catch(() => {})
}

async function createShortcut(item: Instance) {
  try {
    await invoke('create_desktop_shortcut', { id: item.id })
    toast.add({ title: t('instance.shortcutCreated'), color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function duplicate(item: Instance) {
  try {
    await invoke('duplicate_instance', { id: item.id })
    await instances.load()
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function openFolder(item: Instance) {
  try {
    await invoke('open_instance_folder', { id: item.id })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function copyPath(item: Instance) {
  try {
    const path = await invoke<string>('get_instance_path', { id: item.id })
    await navigator.clipboard.writeText(path)
    toast.add({ title: t('library.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

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

const addons = useAddonsStore()

function addonMenu(item: Instance) {
  return addons.buttons('instance.menu').map(b => ({
    label: addons.text(b.addonId, b.title, locale.value),
    ...(b.icon ? { avatar: { src: convertFileSrc(b.icon) } } : { icon: 'i-lucide-puzzle' }),
    onSelect: async () => {
      try {
        if (!await runAddonButton(b, { instanceId: item.id })) toast.add({ title: t('addons.commandMissing'), color: 'warning' })
      } catch (e) {
        toast.add({ title: errorText(e), color: 'error' })
      }
    },
  }))
}

function instanceMenu(item: Instance) {
  const extra = addonMenu(item)
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
    ...(extra.length ? [extra] : []),
    [
      { label: t('common.remove'), icon: 'i-lucide-trash-2', color: 'error' as const, onSelect: () => instances.remove(item.id) },
    ],
  ]
}

const contextItems = computed(() =>
  contextTarget.value ? instanceMenu(contextTarget.value) : emptyMenu.value,
)

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
.mk-card-ghost {
  opacity: 0.4;
  border-style: dashed;
  border-color: var(--ui-primary);
  background: color-mix(in oklab, var(--ui-primary) 8%, transparent);
}
.mk-group-ghost {
  opacity: 0.5;
}

.sortable-fallback {
  opacity: 1 !important;
  background: var(--ui-bg-elevated, #1c1c1f) !important;
  backdrop-filter: none !important;
  border-color: var(--ui-primary) !important;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.55);
}
</style>
