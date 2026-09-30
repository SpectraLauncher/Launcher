<template>
  <div class="space-y-6">
    <div>
      <div class="flex items-center justify-between gap-2">
        <p class="text-sm font-medium">{{ t('addons.installed') }}</p>
        <div class="flex items-center gap-1">
          <UButton
            icon="i-lucide-refresh-cw"
            size="xs"
            color="neutral"
            variant="ghost"
            :loading="checking"
            :label="t('addons.checkUpdates')"
            @click="checkUpdates"
          />
          <UButton
            icon="i-lucide-folder-open"
            size="xs"
            color="neutral"
            variant="ghost"
            :label="t('addons.openFolder')"
            @click="run(() => invoke<void>('addons_open_folder'))"
          />
        </div>
      </div>
      <p v-if="!addons.addons.length" class="mt-2 text-sm text-muted">{{ t('addons.none') }}</p>
      <ul v-else class="mt-2 space-y-2">
        <li
          v-for="a in addons.addons"
          :key="a.id"
          class="rounded-xl border border-default px-3 py-2"
        >
          <div class="flex items-center gap-3">
            <img v-if="a.icon" :src="a.icon" alt="" class="size-9 shrink-0 rounded-lg object-cover">
            <div v-else class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-white/5">
              <UIcon name="i-lucide-puzzle" class="size-4" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <p class="truncate text-sm font-medium">{{ a.name }}</p>
                <span class="font-mono text-xs text-muted">{{ a.version }}</span>
                <UBadge v-if="a.source !== 'catalog'" color="warning" variant="soft" size="sm" :label="t('addons.unverified')" />
                <UBadge v-if="a.revoked" color="error" variant="soft" size="sm" :label="t('addons.revoked')" />
              </div>
              <p v-if="a.error" class="text-xs text-error">{{ a.error }}</p>
              <p v-else-if="a.description" class="truncate text-xs text-muted">{{ a.description }}</p>
            </div>
            <UButton
              v-if="a.settings && a.enabled && !a.error"
              icon="i-lucide-sliders-horizontal"
              size="xs"
              color="neutral"
              :variant="openSettings === a.id ? 'soft' : 'ghost'"
              :title="t('addons.openSettings')"
              :aria-label="t('addons.openSettings')"
              @click="openSettings = openSettings === a.id ? null : a.id"
            />
            <UButton
              v-if="a.source === 'folder' && devMode"
              icon="i-lucide-rotate-cw"
              size="xs"
              color="neutral"
              variant="ghost"
              :title="t('addons.reload')"
              :aria-label="t('addons.reload')"
              @click="reload(a)"
            />
            <UButton
              v-if="addons.updateFor(a.id) && a.project"
              icon="i-lucide-arrow-up-circle"
              size="xs"
              :label="t('addons.updateTo', { version: addons.updateFor(a.id)!.latest })"
              @click="install.fromCatalog(a.project)"
            />
            <USwitch
              :model-value="a.enabled"
              :disabled="a.revoked"
              :aria-label="t('addons.enabled')"
              @update:model-value="(v: boolean) => toggle(a, v)"
            />
            <UButton
              icon="i-lucide-trash-2"
              size="xs"
              color="error"
              variant="ghost"
              :title="t('addons.uninstall')"
              :aria-label="t('addons.uninstall')"
              @click="remove(a)"
            />
          </div>
          <AddonFrame
            v-if="openSettings === a.id && a.settings && a.enabled && !a.error"
            :key="`${a.id}@${a.version}`"
            :addon-id="a.id"
            :entry="a.settings"
            :title="a.name"
            class="mt-3 h-80 w-full rounded-lg border border-default"
          />
        </li>
      </ul>
    </div>

    <div v-if="devMode" class="flex flex-wrap gap-2">
      <UButton icon="i-lucide-folder-open" variant="soft" :label="t('addons.loadFolder')" @click="pickFolder" />
      <UButton icon="i-lucide-file-archive" variant="soft" :label="t('addons.loadZip')" @click="pickZip" />
    </div>

    <div>
      <p class="text-sm font-medium">{{ t('addons.browse') }}</p>
      <div class="mt-2 flex items-center gap-2">
        <UInput v-model="query" icon="i-lucide-search" :placeholder="t('addons.search')" class="w-full max-w-sm" />
        <div class="ml-auto flex items-center gap-1">
          <UButton
            size="xs"
            square
            icon="i-lucide-layout-grid"
            :color="view === 'grid' ? 'primary' : 'neutral'"
            :variant="view === 'grid' ? 'subtle' : 'ghost'"
            :title="t('addons.viewGrid')"
            :aria-label="t('addons.viewGrid')"
            @click="view = 'grid'"
          />
          <UButton
            size="xs"
            square
            icon="i-lucide-list"
            :color="view === 'list' ? 'primary' : 'neutral'"
            :variant="view === 'list' ? 'subtle' : 'ghost'"
            :title="t('addons.viewList')"
            :aria-label="t('addons.viewList')"
            @click="view = 'list'"
          />
        </div>
      </div>
      <p v-if="closed" class="mt-3 text-sm text-muted">{{ t('addons.closed') }}</p>
      <p v-else-if="searched && !hits.length" class="mt-3 text-sm text-muted">{{ t('addons.nothingFound') }}</p>
      <div v-else-if="view === 'grid'" class="mt-3 grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-3">
        <button
          v-for="hit in hits"
          :key="hit.slug"
          type="button"
          class="flex flex-col gap-3 rounded-xl border border-default p-3 text-left transition hover:border-primary-500/40 hover:bg-white/3"
          @click="previewSlug = hit.slug"
        >
          <div class="flex items-center gap-3">
            <img v-if="hit.icon" :src="hit.icon" alt="" class="size-11 shrink-0 rounded-lg object-cover">
            <div v-else class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-white/5">
              <UIcon name="i-lucide-puzzle" class="size-5" />
            </div>
            <p class="min-w-0 truncate text-sm font-medium">{{ hit.title }}</p>
          </div>
          <p class="line-clamp-2 min-h-8 text-xs text-muted">{{ hit.summary }}</p>
          <div class="mt-auto flex items-center justify-between gap-2">
            <span class="text-xs text-dimmed">
              <UIcon name="i-lucide-download" class="mr-1 size-3 align-[-1px]" />{{ count(hit.downloads ?? 0) }}
            </span>
            <UBadge
              v-if="installedProjects.has(hit.slug)"
              size="sm"
              variant="soft"
              color="success"
              :label="t('addons.installed')"
            />
          </div>
        </button>
      </div>
      <ul v-else class="mt-3 space-y-2">
        <li
          v-for="hit in hits"
          :key="hit.slug"
          class="flex cursor-pointer items-center gap-3 rounded-xl border border-default px-3 py-2 transition hover:border-primary-500/40 hover:bg-white/3"
          @click="previewSlug = hit.slug"
        >
          <img v-if="hit.icon" :src="hit.icon" alt="" class="size-9 shrink-0 rounded-lg object-cover">
          <div v-else class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-white/5">
            <UIcon name="i-lucide-puzzle" class="size-4" />
          </div>
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium">{{ hit.title }}</p>
            <p class="truncate text-xs text-muted">{{ hit.summary }}</p>
          </div>
          <UButton
            size="xs"
            :variant="installedProjects.has(hit.slug) ? 'soft' : 'solid'"
            :label="installedProjects.has(hit.slug) ? t('addons.update') : t('addons.install')"
            @click.stop="install.fromCatalog(hit.slug)"
          />
        </li>
      </ul>
    </div>

    <AddonPreview
      v-model:slug="previewSlug"
      :installed="!!previewSlug && installedProjects.has(previewSlug)"
      @install="installFromPreview"
    />
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { Addon } from '~/types/launcher'

defineProps<{ devMode: boolean }>()

interface CatalogHit {
  slug: string
  title: string
  summary: string
  icon: string | null
  downloads?: number
}

const { t, locale } = useI18n()
const toast = useToast()
const addons = useAddonsStore()
const install = useAddonInstall()

const openSettings = ref<string | null>(null)
const checking = ref(false)

async function checkUpdates() {
  checking.value = true
  try {
    const updates = await addons.checkUpdates()
    if (!updates.length) toast.add({ title: t('addons.upToDate'), color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    checking.value = false
  }
}
const query = ref('')
const hits = ref<CatalogHit[]>([])
const closed = ref(false)
const searched = ref(false)

const installedProjects = computed(() => new Set(addons.addons.map(a => a.project).filter(Boolean)))

const VIEW_KEY = 'spectra-addons-view'
const view = ref<'grid' | 'list'>((() => {
  try {
    return localStorage.getItem(VIEW_KEY) === 'list' ? 'list' : 'grid'
  } catch {
    return 'grid'
  }
})())
watch(view, (value) => {
  try {
    localStorage.setItem(VIEW_KEY, value)
  } catch {
  }
})

const previewSlug = ref<string | null>(null)

function installFromPreview(slug: string) {
  previewSlug.value = null
  void install.fromCatalog(slug)
}

const count = (n: number) => new Intl.NumberFormat(locale.value).format(n)

async function search() {
  try {
    const res = await invoke<{ hits?: CatalogHit[]; closed?: boolean }>('addons_catalog', { query: query.value })
    hits.value = res.hits ?? []
    closed.value = Boolean(res.closed)
  } catch (e) {
    hits.value = []
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    searched.value = true
  }
}

let searchTimer: ReturnType<typeof setTimeout> | undefined
watch(query, () => {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(search, 300)
})

onMounted(() => {
  addons.load().catch(() => {})
  search()
})

async function run(action: () => Promise<void>, done?: string) {
  try {
    await action()
    if (done) toast.add({ title: done, color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

const toggle = (a: Addon, enabled: boolean) => run(() => addons.setEnabled(a.id, enabled))
const remove = (a: Addon) => run(() => addons.uninstall(a.id), t('addons.uninstalled', { name: a.name }))
const reload = (a: Addon) => run(() => addons.reload(a.id), t('addons.reloaded', { name: a.name }))

async function pickFolder() {
  const path = await open({ directory: true, multiple: false })
  if (typeof path === 'string') install.fromFolder(path)
}

async function pickZip() {
  const path = await open({ multiple: false, directory: false, filters: [{ name: 'Addon', extensions: ['zip'] }] })
  if (typeof path === 'string') install.fromFile(path)
}
</script>
