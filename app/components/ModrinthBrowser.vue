<template>
  <UModal
    v-model:open="isOpen"
    :title="title"
    :ui="{ content: 'max-w-5xl w-[92vw] h-[82vh]', body: 'p-0 overflow-hidden' }"
  >
    <template #body>
      <div class="flex h-full flex-col">
        <!-- toolbar -->
        <div class="space-y-2 border-b border-default px-4 py-3">
          <div class="flex flex-wrap items-center gap-2">
            <USelect
              v-if="providerItems.length > 1"
              v-model="provider"
              :items="providerItems"
              value-key="value"
              class="w-32"
            />
            <UInput
              v-model="query"
              icon="i-lucide-search"
              variant="soft"
              :placeholder="$t('modrinth.searchPlaceholder')"
              class="min-w-48 flex-1"
            />
            <USelectMenu
              v-model="gameVersion"
              :items="gameVersionItems"
              value-key="value"
              :placeholder="$t('modrinth.anyVersion')"
              class="w-40"
            />
            <USelect
              v-if="showLoaderFilter"
              v-model="loader"
              :items="loaderItems"
              value-key="value"
              class="w-36"
            />
            <USelect v-model="sort" :items="sortItems" value-key="value" class="w-36" />
            <UButton
              icon="i-lucide-sliders-horizontal"
              :color="showFilters ? 'primary' : 'neutral'"
              variant="soft"
              square
              :title="$t('modrinth.filters')"
              @click="showFilters = !showFilters"
            />
          </div>

          <!-- category chips -->
          <div v-if="showFilters" class="flex flex-wrap gap-1.5 pt-1">
            <button
              v-for="c in categories"
              :key="c.name"
              type="button"
              class="rounded-md px-2 py-1 text-xs font-medium capitalize transition"
              :class="selectedCategories.includes(c.name)
                ? 'bg-primary-500/15 text-primary-400'
                : 'bg-white/5 text-neutral-400 hover:bg-white/10'"
              @click="toggleCategory(c.name)"
            >
              {{ c.name }}
            </button>
            <span v-if="!categories.length" class="text-xs text-muted">{{ $t('common.loading') }}</span>
          </div>
        </div>

        <!-- body: results + detail -->
        <div class="flex min-h-0 flex-1">
          <!-- results -->
          <div class="w-2/5 min-w-72 overflow-y-auto border-r border-default">
            <p v-if="searchError" class="p-4 text-sm text-error">{{ searchError }}</p>
            <div v-else-if="loadingSearch && !hits.length" class="p-4 text-sm text-muted">{{ $t('common.loading') }}</div>
            <div v-else-if="!hits.length" class="p-8 text-center text-sm text-muted">{{ $t('modrinth.noResults') }}</div>

            <button
              v-for="hit in hits"
              :key="hit.project_id"
              type="button"
              class="flex w-full items-start gap-3 border-b border-default/60 p-3 text-left transition"
              :class="selected?.project_id === hit.project_id ? 'bg-primary-500/10' : 'hover:bg-white/4'"
              @click="selectHit(hit)"
            >
              <img v-if="hit.icon_url" :src="hit.icon_url" class="size-11 shrink-0 rounded-lg object-cover" :alt="hit.title" />
              <div v-else class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-white/5">
                <UIcon name="i-lucide-box" class="size-5 text-neutral-500" />
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <span class="truncate text-sm font-semibold">{{ hit.title }}</span>
                  <UIcon
                    v-if="installedIds.has(hit.project_id)"
                    name="i-lucide-circle-check"
                    class="size-3.5 shrink-0 text-success"
                    :title="$t('modrinth.installedShort')"
                  />
                </div>
                <div class="truncate text-[11px] text-neutral-500">{{ $t('modrinth.by', { author: hit.author }) }}</div>
                <p class="mt-1 line-clamp-2 text-[12px] text-neutral-400">{{ hit.description }}</p>
                <div class="mt-1 flex items-center gap-1 text-[11px] text-neutral-500">
                  <UIcon name="i-lucide-download" class="size-3" />{{ compactNumber(hit.downloads) }}
                </div>
              </div>
            </button>

            <div v-if="hits.length && hits.length < totalHits" class="p-3">
              <UButton
                block
                color="neutral"
                variant="soft"
                size="sm"
                :loading="loadingSearch"
                :label="$t('modrinth.loadMore')"
                @click="loadMore"
              />
            </div>
          </div>

          <!-- detail -->
          <div class="min-w-0 flex-1 overflow-y-auto">
            <div v-if="!selected" class="flex h-full items-center justify-center p-8 text-center text-sm text-muted">
              {{ $t('modrinth.selectHint') }}
            </div>
            <div v-else class="p-4">
              <!-- name row: title on the left, install / manage on the right -->
              <div class="flex items-start justify-between gap-3">
                <div class="flex min-w-0 items-start gap-3">
                  <img v-if="selected.icon_url" :src="selected.icon_url" class="size-14 shrink-0 rounded-xl object-cover" :alt="selected.title" />
                  <div class="min-w-0">
                    <div class="flex items-center gap-2">
                      <button
                        type="button"
                        class="group/title flex min-w-0 items-center gap-1.5 text-left transition hover:text-primary-400"
                        :title="$t('mods.openPage')"
                        @click="openProjectPage(selected)"
                      >
                        <span class="truncate text-lg font-bold">{{ selected.title }}</span>
                        <UIcon name="i-lucide-external-link" class="size-4 shrink-0 opacity-0 transition group-hover/title:opacity-100" />
                      </button>
                      <UBadge v-if="installedIds.has(selected.project_id)" color="success" variant="subtle" size="xs" :label="$t('modrinth.installedShort')" />
                    </div>
                    <div class="text-xs text-neutral-500">{{ $t('modrinth.by', { author: selected.author }) }}</div>
                  </div>
                </div>
                <!-- installed mod: change version + uninstall; otherwise: install -->
                <div v-if="selectedInstalled" class="flex shrink-0 items-center gap-1.5">
                  <UButton
                    icon="i-lucide-replace"
                    color="neutral"
                    variant="soft"
                    :disabled="loadingVersions || !versions.length || uninstalling"
                    :label="$t('mods.changeVersion')"
                    @click="showAllVersions = true"
                  />
                  <UButton
                    icon="i-lucide-trash-2"
                    color="error"
                    variant="soft"
                    square
                    :loading="uninstalling"
                    :title="$t('common.remove')"
                    @click="doUninstall"
                  />
                </div>
                <UButton
                  v-else
                  class="shrink-0"
                  :loading="!!installing"
                  :disabled="!latest || loadingVersions"
                  :label="installLabel"
                  @click="latest && doInstall(latest)"
                />
              </div>

              <p class="mt-2 text-[13px] text-neutral-400">{{ selected.description }}</p>
              <p v-if="latest" class="mt-1 text-[11px] text-neutral-500">
                {{ $t('modrinth.latest') }}: <span class="font-mono">{{ latest.version_number }}</span>
              </p>

              <!-- modpack install progress -->
              <div v-if="modpackProgress" class="mt-4 space-y-1.5">
                <div class="flex justify-between text-xs text-muted">
                  <span>{{ $t('modrinth.installingModpack') }}</span>
                  <span>{{ modpackProgress.current }} / {{ modpackProgress.total }}</span>
                </div>
                <UProgress :model-value="modpackProgress.current" :max="modpackProgress.total || 1" />
              </div>

              <!-- choose another (older) version -->
              <button
                v-if="versions.length > 1"
                type="button"
                class="mt-5 flex items-center gap-1.5 text-sm text-muted transition hover:text-default"
                @click="showAllVersions = !showAllVersions"
              >
                <UIcon :name="showAllVersions ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'" class="size-4" />
                {{ $t('modrinth.chooseVersion') }}
              </button>

              <div v-if="loadingVersions" class="py-6 text-center text-sm text-muted">{{ $t('common.loading') }}</div>
              <div v-else-if="!versions.length" class="py-6 text-center text-sm text-muted">{{ $t('modrinth.noVersions') }}</div>
              <div v-else-if="showAllVersions" class="mt-3 space-y-2">
                <div
                  v-for="v in versions"
                  :key="v.id"
                  class="flex items-center gap-3 rounded-lg border border-default bg-white/3 p-2.5"
                >
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <span class="truncate text-sm font-medium">{{ v.name || v.version_number }}</span>
                      <UBadge :color="versionTypeColor(v.version_type)" variant="subtle" size="xs" :label="v.version_type" />
                    </div>
                    <div class="mt-0.5 truncate text-[11px] text-neutral-500">
                      {{ v.game_versions.slice(0, 4).join(', ') }}
                      <span v-if="v.loaders.length"> · {{ v.loaders.join(', ') }}</span>
                    </div>
                  </div>
                  <UButton
                    size="xs"
                    color="neutral"
                    variant="soft"
                    :loading="installing === v.id"
                    :disabled="!!installing"
                    :label="installLabel"
                    @click="doInstall(v)"
                  />
                </div>
              </div>

              <!-- description / gallery tabs -->
              <div class="mt-5 border-t border-default pt-3">
                <div class="mb-3 flex gap-1.5">
                  <button
                    type="button"
                    class="rounded-md px-3 py-1 text-sm font-medium transition"
                    :class="detailTab === 'description' ? 'bg-primary-500/15 text-primary-400' : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
                    @click="detailTab = 'description'"
                  >
                    {{ $t('modrinth.tabDescription') }}
                  </button>
                  <button
                    type="button"
                    class="flex items-center gap-1.5 rounded-md px-3 py-1 text-sm font-medium transition"
                    :class="detailTab === 'gallery' ? 'bg-primary-500/15 text-primary-400' : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
                    @click="detailTab = 'gallery'"
                  >
                    {{ $t('modrinth.tabGallery') }}
                    <span v-if="gallery.length" class="text-[11px] text-neutral-500">{{ gallery.length }}</span>
                  </button>
                </div>

                <div v-if="loadingBody" class="py-8 text-center text-sm text-muted">{{ $t('common.loading') }}</div>

                <!-- description -->
                <div v-else-if="detailTab === 'description'">
                  <div v-if="bodyHtml" class="mk-md" v-html="bodyHtml" />
                  <p v-else class="py-6 text-center text-sm text-muted">{{ selected.description }}</p>
                </div>

                <!-- gallery -->
                <div v-else>
                  <div v-if="!gallery.length" class="py-8 text-center text-sm text-muted">{{ $t('modrinth.noGallery') }}</div>
                  <div v-else class="grid grid-cols-2 gap-2">
                    <button
                      v-for="(g, i) in gallery"
                      :key="g.url"
                      type="button"
                      class="overflow-hidden rounded-lg border border-default bg-white/3 transition hover:border-primary-500/40"
                      @click="galleryIndex = i"
                    >
                      <img :src="g.url" loading="lazy" class="aspect-video w-full object-cover" :alt="g.title ?? ''" >
                      <div v-if="g.title" class="truncate px-2 py-1 text-left text-[11px] text-neutral-400">{{ g.title }}</div>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </UModal>

  <!-- gallery lightbox -->
  <UModal v-model:open="galleryOpen" :ui="{ content: 'max-w-5xl w-[92vw]' }">
    <template #content>
      <div v-if="galleryImage" class="flex flex-col">
        <div class="flex items-center justify-between gap-2 border-b border-default px-4 py-2.5">
          <div class="min-w-0">
            <div class="truncate text-sm font-medium">{{ galleryImage.title || `${(galleryIndex ?? 0) + 1} / ${gallery.length}` }}</div>
            <div v-if="galleryImage.description" class="truncate text-xs text-muted">{{ galleryImage.description }}</div>
          </div>
          <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" square @click="galleryIndex = null" />
        </div>
        <div class="relative bg-black/40">
          <img :src="galleryImage.raw_url || galleryImage.url" class="max-h-[78vh] w-full object-contain" :alt="galleryImage.title ?? ''" >
          <UButton v-if="gallery.length > 1" icon="i-lucide-chevron-left" color="neutral" class="absolute top-1/2 left-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100" @click="stepGallery(-1)" />
          <UButton v-if="gallery.length > 1" icon="i-lucide-chevron-right" color="neutral" class="absolute top-1/2 right-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100" @click="stepGallery(1)" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { openExternal as openUrl } from '~/utils/openExternal'
import { marked } from 'marked'
import type { ModrinthHit, ModrinthVersion, ModrinthCategory, ModrinthSortIndex, ModrinthGalleryItem, ModrinthProjectType, InstalledItem } from '~/types/modrinth'
import type { LoaderType } from '~/types/launcher'

const { isOpen, config, close } = useModrinthBrowser()
const modrinth = useModrinth()
const curseforge = useCurseforge()
const blockedModal = useBlockedModsModal()
const meta = useMinecraftMeta()

// --- provider (Modrinth / CurseForge) ---
type Provider = 'modrinth' | 'curseforge'
const provider = ref<Provider>('modrinth')
const cfEnabled = ref(false)
const isCf = computed(() => provider.value === 'curseforge')
const providerItems = computed(() => {
  const items = [{ label: 'Modrinth', value: 'modrinth' as Provider }]
  if (cfEnabled.value) items.push({ label: 'CurseForge', value: 'curseforge' as Provider })
  return items
})
const instances = useInstancesStore()
const activity = useActivityCenter()
const toast = useToast()
const { t } = useI18n()

const kind = computed(() => config.value?.kind ?? 'mod')
const mode = computed(() => config.value?.mode ?? 'install')
const showLoaderFilter = computed(() => usesLoaderFilter(kind.value))

const title = computed(() => t(`modrinth.title.${kind.value}`))
const installLabel = computed(() =>
  mode.value === 'createModpack' ? t('modrinth.createInstance') : t('modrinth.install'),
)

// --- filters ---
// "any" is a sentinel — Reka UI's Select forbids an empty-string item value.
const ANY = 'any'
const query = ref('')
const sort = ref<ModrinthSortIndex>('relevance')
const gameVersion = ref<string>(ANY)
const loader = ref<LoaderType | typeof ANY>(ANY)
const selectedCategories = ref<string[]>([])
const showFilters = ref(false)

const gameVersionValue = computed(() => (gameVersion.value === ANY ? undefined : gameVersion.value))
const loaderValue = computed(() => (loader.value === ANY ? undefined : loader.value))

const sortItems = computed(() => (['relevance', 'downloads', 'follows', 'newest', 'updated'] as const).map(v => ({
  label: t(`modrinth.sort.${v}`),
  value: v,
})))
const loaderItems = computed(() => [
  { label: t('modrinth.anyLoader'), value: ANY },
  { label: 'Fabric', value: 'fabric' },
  { label: 'NeoForge', value: 'neoforge' },
  { label: 'Forge', value: 'forge' },
  { label: 'Quilt', value: 'quilt' },
])

const mcVersions = ref<string[]>([])
const gameVersionItems = computed(() => [
  { label: t('modrinth.anyVersion'), value: ANY },
  ...mcVersions.value.map(v => ({ label: v, value: v })),
])

// --- results ---
const hits = ref<ModrinthHit[]>([])
const totalHits = ref(0)
const offset = ref(0)
const loadingSearch = ref(false)
const searchError = ref<string | null>(null)
const LIMIT = 20

const categories = ref<ModrinthCategory[]>([])

// --- detail ---
const selected = ref<ModrinthHit | null>(null)
const versions = ref<ModrinthVersion[]>([])
const loadingVersions = ref(false)
const installing = ref<string | null>(null)
const uninstalling = ref(false)
const modpackProgress = ref<{ current: number; total: number } | null>(null)
const showAllVersions = ref(false)

// Full markdown description (Modrinth project `body`) + gallery.
const bodyHtml = ref('')
const loadingBody = ref(false)
const detailTab = ref<'description' | 'gallery'>('description')
const gallery = ref<ModrinthGalleryItem[]>([])

// Gallery lightbox.
const galleryIndex = ref<number | null>(null)
const galleryImage = computed(() => (galleryIndex.value !== null ? gallery.value[galleryIndex.value] ?? null : null))
const galleryOpen = computed({
  get: () => galleryIndex.value !== null,
  set: (v: boolean) => { if (!v) galleryIndex.value = null },
})
function stepGallery(delta: number) {
  if (galleryIndex.value === null || !gallery.value.length) return
  const n = gallery.value.length
  galleryIndex.value = (galleryIndex.value + delta + n) % n
}

async function renderBody(md: string) {
  if (!md) {
    bodyHtml.value = ''
    return
  }
  const raw = marked.parse(md, { async: false }) as string
  const DOMPurify = (await import('dompurify')).default
  bodyHtml.value = DOMPurify.sanitize(raw, { ADD_ATTR: ['target'] })
}

// project_ids already installed in the target instance, plus the full records
// (keyed by project_id) so we can uninstall a mod straight from the browser.
const installedIds = ref<Set<string>>(new Set())
const installedItems = ref<Map<string, InstalledItem>>(new Map())

// The current selection, if it's already installed and can be removed here.
// Change-version / uninstall from the browser is a mod-only affordance.
const selectedInstalled = computed(() =>
  selected.value && kind.value === 'mod' ? installedItems.value.get(selected.value.project_id) ?? null : null,
)

const latest = computed<ModrinthVersion | null>(() =>
  versions.value.length
    ? [...versions.value].sort((a, b) => b.date_published.localeCompare(a.date_published))[0]!
    : null,
)

async function loadInstalled() {
  const cfg = config.value
  if (cfg?.mode === 'install' && cfg.instanceId) {
    try {
      const items = await modrinth.getInstalled(cfg.instanceId)
      installedIds.value = new Set(items.map(i => i.project_id))
      installedItems.value = new Map(items.map(i => [i.project_id, i]))
    } catch {
      installedIds.value = new Set()
      installedItems.value = new Map()
    }
  } else {
    installedIds.value = new Set()
    installedItems.value = new Map()
  }
}

async function runSearch(append = false) {
  if (!config.value) return
  loadingSearch.value = true
  searchError.value = null
  try {
    const params = {
      query: query.value,
      // CurseForge keeps the raw kind (its classId distinguishes datapacks);
      // Modrinth collapses datapack → mod + a category facet. The CF backend
      // accepts the raw kind as a plain string; the cast just satisfies the
      // shared param type (Modrinth never receives "datapack" at runtime).
      project_type: (isCf.value ? kind.value : searchProjectType(kind.value)) as ModrinthProjectType,
      loaders: loaderFacetFor(kind.value, loaderValue.value) || [],
      game_versions: gameVersionValue.value ? [gameVersionValue.value] : [],
      categories: isCf.value
        ? selectedCategories.value
        : [...baseCategories(kind.value), ...selectedCategories.value],
      index: sort.value,
      offset: append ? offset.value : 0,
      limit: LIMIT,
    }
    const res = await (isCf.value ? curseforge.search(params) : modrinth.search(params))
    totalHits.value = res.total_hits
    offset.value = res.offset + res.hits.length
    hits.value = append ? [...hits.value, ...res.hits] : res.hits
  } catch (e) {
    searchError.value = String(e)
  } finally {
    loadingSearch.value = false
  }
}

function loadMore() {
  runSearch(true)
}

// Debounced re-search when any filter changes.
let debounce: ReturnType<typeof setTimeout> | undefined
watch([query, sort, gameVersion, loader, selectedCategories], () => {
  clearTimeout(debounce)
  debounce = setTimeout(() => runSearch(false), 300)
}, { deep: true })

function toggleCategory(name: string) {
  selectedCategories.value = selectedCategories.value.includes(name)
    ? selectedCategories.value.filter(c => c !== name)
    : [...selectedCategories.value, name]
}

async function selectHit(hit: ModrinthHit) {
  selected.value = hit
  versions.value = []
  showAllVersions.value = false
  loadingVersions.value = true
  bodyHtml.value = ''
  gallery.value = []
  galleryIndex.value = null
  detailTab.value = 'description'
  loadingBody.value = true

  const api = isCf.value ? curseforge : modrinth

  // Full project: markdown description + gallery (in parallel with versions).
  api.project(hit.project_id)
    .then((p) => {
      gallery.value = p.gallery ?? []
      return renderBody(p.body)
    })
    .catch(() => { bodyHtml.value = '' })
    .finally(() => { loadingBody.value = false })

  try {
    versions.value = await api.versions(
      hit.project_id,
      loaderFacetFor(kind.value, loaderValue.value) || undefined,
      gameVersionValue.value ? [gameVersionValue.value] : undefined,
    )
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    loadingVersions.value = false
  }
}

async function doInstall(version: ModrinthVersion) {
  const cfg = config.value
  if (!cfg) return
  const name = selected.value?.title ?? ''
  // When re-installing a different version of an already-installed mod, remember
  // the previous jar so we can remove it once the new one lands (a version swap).
  const prevInstalled = selectedInstalled.value
  installing.value = version.id
  const taskId = activity.startTask(
    cfg.mode === 'createModpack' ? t('activity.installingModpack', { name }) : t('activity.installingMod', { name }),
  )
  try {
    if (cfg.mode === 'createModpack') {
      modpackProgress.value = { current: 0, total: 0 }
      let instance
      if (isCf.value) {
        instance = await curseforge.installModpack(version.project_id, version.id, null)
      } else {
        const file = version.files.find(f => f.primary) ?? version.files[0]
        if (!file) throw new Error(t('modrinth.noFile'))
        instance = await modrinth.installModpack(
          file.url,
          null,
          selected.value?.icon_url ?? null,
          selected.value?.project_id ?? null,
          version.id,
        )
      }
      await instances.load()
      toast.add({ title: t('modrinth.installed', { name: instance.name }), color: 'success' })
      cfg.onInstalled?.(instance)
      close()
      // CurseForge packs may contain mods the author blocked from auto-download.
      if (isCf.value) {
        const blocked = await curseforge.getBlocked(instance.id)
        if (blocked.length) blockedModal.open(instance.id)
      }
    } else if (cfg.instanceId) {
      if (isCf.value) {
        const res = await curseforge.installWithDeps(
          cfg.instanceId,
          version.project_id,
          version.id,
          gameVersionValue.value,
          loaderValue.value,
        )
        installedIds.value = new Set([...installedIds.value, ...res.added.map(i => i.project_id)])
        rememberInstalled(res.added)
        await removeReplacedJar(cfg.instanceId, prevInstalled, res.added)
        const deps = res.added.filter(i => i.dependency).length
        toast.add({
          title: deps > 0 ? t('modrinth.installedWithDeps', { name, n: deps }) : t('modrinth.installed', { name }),
          color: 'success',
        })
        // Authors blocked third-party download → open the manual-download resolver.
        if (res.blocked.length) blockedModal.open(cfg.instanceId)
      } else {
        const added = await modrinth.installWithDeps(
          cfg.instanceId,
          version.id,
          gameVersionValue.value,
          loaderValue.value,
        )
        // Mark installed (incl. dependencies) in the results list.
        installedIds.value = new Set([...installedIds.value, ...added.map(i => i.project_id)])
        rememberInstalled(added)
        await removeReplacedJar(cfg.instanceId, prevInstalled, added)
        const deps = added.filter(i => i.dependency).length
        toast.add({
          title: deps > 0 ? t('modrinth.installedWithDeps', { name, n: deps }) : t('modrinth.installed', { name }),
          color: 'success',
        })
      }
      cfg.onInstalled?.()
    }
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(taskId)
    installing.value = null
    modpackProgress.value = null
  }
}

/** Merges freshly-installed items into the lookup used by the uninstall button. */
function rememberInstalled(items: InstalledItem[]) {
  const next = new Map(installedItems.value)
  for (const it of items) next.set(it.project_id, it)
  installedItems.value = next
}

/** After a version swap, deletes the previous jar if the filename changed. */
async function removeReplacedJar(instanceId: string, prev: InstalledItem | null, added: InstalledItem[]) {
  if (!prev) return
  const fresh = added.find(a => a.project_id === prev.project_id)
  if (fresh && fresh.filename !== prev.filename) {
    try {
      await invoke('delete_mod', { instanceId, filename: prev.filename })
    } catch { /* new jar is in place; a leftover old jar is non-fatal */ }
  }
}

/** Opens the project's page on Modrinth / CurseForge in the system browser. */
function openProjectPage(hit: ModrinthHit) {
  const url = isCf.value
    ? `https://www.curseforge.com/projects/${hit.project_id}`
    : `https://modrinth.com/project/${hit.slug || hit.project_id}`
  openUrl(url).catch(() => { /* nothing to do if the shell can't open it */ })
}

/** Removes the selected mod's jar from the target instance. */
async function doUninstall() {
  const cfg = config.value
  const hit = selected.value
  const item = selectedInstalled.value
  if (!cfg?.instanceId || !hit || !item) return
  const name = item.name || hit.title
  uninstalling.value = true
  const taskId = activity.startTask(t('activity.uninstallingMod', { name }))
  try {
    await invoke('delete_mod', { instanceId: cfg.instanceId, filename: item.filename })
    installedIds.value = new Set([...installedIds.value].filter(id => id !== hit.project_id))
    const next = new Map(installedItems.value)
    next.delete(hit.project_id)
    installedItems.value = next
    toast.add({ title: t('modrinth.uninstalled', { name }), color: 'success' })
    cfg.onInstalled?.()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(taskId)
    uninstalling.value = false
  }
}

function versionTypeColor(type: string) {
  if (type === 'release') return 'success' as const
  if (type === 'beta') return 'warning' as const
  return 'neutral' as const
}

// Initialise from config each time the modal opens.
watch(isOpen, async (open) => {
  if (!open || !config.value) return
  const cfg = config.value
  provider.value = 'modrinth'
  curseforge.enabled().then(v => (cfEnabled.value = v)).catch(() => (cfEnabled.value = false))
  query.value = cfg.query ?? ''
  sort.value = 'relevance'
  gameVersion.value = cfg.gameVersion ?? ANY
  loader.value = usesLoaderFilter(cfg.kind) ? (cfg.loader ?? ANY) : ANY
  selectedCategories.value = []
  showFilters.value = false
  selected.value = null
  versions.value = []
  hits.value = []
  loadInstalled()

  if (!mcVersions.value.length) {
    try {
      mcVersions.value = (await meta.getMinecraftVersions(true)).map(v => v.id)
    } catch { /* keep "any" only */ }
  }
  loadCategories()
  runSearch(false)
})

function loadCategories() {
  const k = config.value?.kind ?? 'mod'
  categories.value = []
  const p = isCf.value ? curseforge.categories(k) : modrinth.categories(searchProjectType(k))
  p.then(c => (categories.value = c)).catch(() => { categories.value = [] })
}

// Switching provider resets results, selection and reloads that provider's chips.
watch(provider, () => {
  if (!isOpen.value) return
  selected.value = null
  versions.value = []
  hits.value = []
  selectedCategories.value = []
  showFilters.value = false
  loadCategories()
  runSearch(false)
})

// Modpack install progress events.
let unlisten: UnlistenFn | null = null
onMounted(async () => {
  unlisten = await listen<{ current: number; total: number }>('modrinth://modpack-progress', (e) => {
    modpackProgress.value = { current: e.payload.current, total: e.payload.total }
  })
})
onBeforeUnmount(() => unlisten?.())

// Keyboard navigation for the gallery lightbox.
function onGalleryKey(e: KeyboardEvent) {
  if (galleryIndex.value === null) return
  if (e.key === 'ArrowRight') { e.preventDefault(); stepGallery(1) }
  else if (e.key === 'ArrowLeft') { e.preventDefault(); stepGallery(-1) }
  else if (e.key === 'Escape') galleryIndex.value = null
}
onMounted(() => window.addEventListener('keydown', onGalleryKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onGalleryKey))
</script>

<style>
/* Minimal prose styling for the rendered Modrinth markdown description. */
.mk-md {
  font-size: 13px;
  line-height: 1.6;
  color: var(--ui-text-muted);
  word-break: break-word;
}
.mk-md h1, .mk-md h2, .mk-md h3, .mk-md h4 {
  color: var(--ui-text-highlighted);
  font-weight: 600;
  line-height: 1.3;
  margin: 1em 0 0.4em;
}
.mk-md h1 { font-size: 1.3em; }
.mk-md h2 { font-size: 1.15em; }
.mk-md h3 { font-size: 1.05em; }
.mk-md p { margin: 0.6em 0; }
.mk-md a { color: var(--ui-primary); text-decoration: underline; }
.mk-md ul, .mk-md ol { margin: 0.6em 0; padding-left: 1.4em; }
.mk-md ul { list-style: disc; }
.mk-md ol { list-style: decimal; }
.mk-md li { margin: 0.2em 0; }
.mk-md img { max-width: 100%; height: auto; border-radius: 8px; margin: 0.5em 0; }
.mk-md code {
  font-family: ui-monospace, monospace;
  font-size: 0.9em;
  background: rgba(255, 255, 255, 0.08);
  padding: 0.1em 0.35em;
  border-radius: 4px;
}
.mk-md pre {
  background: rgba(0, 0, 0, 0.35);
  padding: 0.8em;
  border-radius: 8px;
  overflow-x: auto;
  margin: 0.7em 0;
}
.mk-md pre code { background: none; padding: 0; }
.mk-md blockquote {
  border-left: 3px solid var(--ui-border-accented);
  padding-left: 0.8em;
  margin: 0.7em 0;
  color: var(--ui-text-dimmed);
}
.mk-md hr { border: 0; border-top: 1px solid var(--ui-border); margin: 1em 0; }
.mk-md table { border-collapse: collapse; margin: 0.7em 0; display: block; overflow-x: auto; }
.mk-md th, .mk-md td { border: 1px solid var(--ui-border); padding: 0.3em 0.6em; }
</style>
