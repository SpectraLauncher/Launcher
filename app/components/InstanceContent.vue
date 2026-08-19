<template>
  <div class="space-y-4">
    <!-- kind selector: one list for mods, packs, shaders and datapacks -->
    <div class="flex flex-wrap items-center gap-1.5">
      <button
        v-for="k in kindTabs"
        :key="k.key"
        type="button"
        class="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium transition"
        :class="kind === k.key
          ? 'bg-primary-500/15 text-primary-400'
          : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
        @click="kind = k.key"
      >
        <UIcon :name="k.icon" class="size-3.5" />
        {{ $t(k.label) }}
        <span v-if="counts[k.key]" class="rounded bg-white/8 px-1 tabular-nums">{{ counts[k.key] }}</span>
      </button>
    </div>

    <!-- toolbar -->
    <div class="flex flex-wrap items-center gap-2">
      <UInput
        v-model="search"
        icon="i-lucide-search"
        variant="soft"
        :placeholder="$t('content.searchPlaceholder')"
        class="min-w-44 flex-1"
      />
      <USelect v-model="perPage" :items="perPageItems" value-key="value" class="w-28" />
      <UButton
        v-if="updateCount"
        icon="i-lucide-circle-arrow-up"
        color="primary"
        size="sm"
        :loading="updatingAll"
        :label="$t('mods.updateAll', { n: updateCount })"
        @click="updateAll"
      />
      <UButton
        v-if="hasLocal"
        icon="i-lucide-link"
        color="neutral"
        variant="soft"
        size="sm"
        :loading="linking"
        :label="$t('mods.link')"
        :title="$t('mods.linkHint')"
        @click="linkLocal"
      />
      <UButton
        v-if="blockedCount"
        icon="i-lucide-file-warning"
        color="warning"
        variant="soft"
        size="sm"
        :label="$t('blocked.resolve', { n: blockedCount })"
        @click="openBlocked"
      />
      <UButton icon="i-lucide-package-search" size="sm" :label="$t('modrinth.add')" @click="openBrowser" />
      <UButton
        v-if="hasMods"
        icon="i-lucide-file-text"
        color="neutral"
        variant="soft"
        size="sm"
        :label="$t('modlist.export')"
        @click="modList.open(props.instanceId)"
      />
      <UButton
        icon="i-lucide-refresh-cw"
        color="neutral"
        variant="ghost"
        size="sm"
        :loading="loading"
        :title="$t('content.refresh')"
        square
        @click="load"
      />
    </div>

    <p v-if="error" class="text-sm text-error">{{ error }}</p>

    <!-- conflicts -->
    <div v-if="conflicts.length" class="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3">
      <div class="flex items-center gap-1.5 text-sm font-medium text-amber-300">
        <UIcon name="i-lucide-triangle-alert" class="size-4" />
        {{ $t('conflicts.title', { n: conflicts.length }) }}
      </div>
      <ul class="mt-1 space-y-0.5 text-xs text-amber-200/80">
        <li v-for="c in conflicts" :key="c.filename + c.kind">
          {{ c.name }} — {{ c.kind === 'loader' ? $t('conflicts.loader', { detail: c.detail }) : $t('conflicts.duplicate') }}
        </li>
      </ul>
    </div>

    <!-- loading skeleton (the conflicts banner above is a sibling, not a branch:
         a conflict must not hide the list) -->
    <div v-if="loading" class="overflow-hidden rounded-xl border border-default">
      <div v-for="n in 8" :key="`mod-sk-${n}`" class="flex items-center gap-3 border-b border-default/50 px-3 py-2.5 last:border-0">
        <div class="size-9 shrink-0 animate-pulse rounded-lg bg-white/5" />
        <div class="min-w-0 flex-1 space-y-2">
          <div class="h-3.5 w-1/3 animate-pulse rounded bg-white/5" />
          <div class="h-2.5 w-1/2 animate-pulse rounded bg-white/5" />
        </div>
        <div class="h-3 w-16 shrink-0 animate-pulse rounded bg-white/5" />
      </div>
    </div>

    <!-- empty: nothing of this kind installed -->
    <div v-else-if="!ofKind.length" class="flex flex-col items-center justify-center gap-3 py-16 text-center">
      <UIcon :name="kindIcon(browserKind)" class="size-10 text-neutral-600" />
      <p class="max-w-sm text-sm text-muted">{{ kind === 'mod' || kind === 'all' ? $t('instance.modsHint') : $t('content.empty') }}</p>
      <UButton icon="i-lucide-package-search" :label="$t('modrinth.add')" @click="openBrowser" />
    </div>

    <!-- no search results -->
    <div v-else-if="!filtered.length" class="py-12 text-center text-sm text-muted">{{ $t('content.empty') }}</div>

    <!-- table -->
    <template v-else>
      <p class="text-xs text-muted">{{ $t('content.count', { n: filtered.length }) }}</p>

      <div class="overflow-hidden rounded-xl border border-default">
        <!-- header (click to sort: asc → desc → off) -->
        <div class="grid grid-cols-[auto_2.75rem_minmax(0,1fr)_8rem_7rem_4rem_auto] items-center gap-3 border-b border-default bg-white/4 px-3 py-2 text-[11px] font-medium tracking-wide text-neutral-500 uppercase">
          <span />
          <button type="button" class="flex items-center justify-center gap-1 transition hover:text-neutral-200" :class="{ 'text-primary-400': sortCol === 'state' }" @click="toggleSort('state')">
            {{ $t('mods.col.enabled') }}
            <UIcon :name="sortIcon('state')" class="size-3" :class="{ 'opacity-30': sortCol !== 'state' }" />
          </button>
          <button type="button" class="flex items-center gap-1 transition hover:text-neutral-200" :class="{ 'text-primary-400': sortCol === 'name' }" @click="toggleSort('name')">
            {{ $t('mods.col.name') }}
            <UIcon :name="sortIcon('name')" class="size-3" :class="{ 'opacity-30': sortCol !== 'name' }" />
          </button>
          <button type="button" class="flex items-center gap-1 transition hover:text-neutral-200" :class="{ 'text-primary-400': sortCol === 'version' }" @click="toggleSort('version')">
            {{ $t('mods.col.version') }}
            <UIcon :name="sortIcon('version')" class="size-3" :class="{ 'opacity-30': sortCol !== 'version' }" />
          </button>
          <button type="button" class="flex items-center gap-1 transition hover:text-neutral-200" :class="{ 'text-primary-400': sortCol === 'updated' }" @click="toggleSort('updated')">
            {{ $t('mods.col.updated') }}
            <UIcon :name="sortIcon('updated')" class="size-3" :class="{ 'opacity-30': sortCol !== 'updated' }" />
          </button>
          <button type="button" class="flex items-center justify-center gap-1 transition hover:text-neutral-200" :class="{ 'text-primary-400': sortCol === 'update' }" :title="$t('mods.col.update')" @click="toggleSort('update')">
            <UIcon name="i-lucide-circle-arrow-up" class="size-3.5" />
            <UIcon :name="sortIcon('update')" class="size-3" :class="{ 'opacity-30': sortCol !== 'update' }" />
          </button>
          <span />
        </div>

        <!-- rows -->
        <div
          v-for="m in paged"
          :key="`${m.kind}/${m.filename}`"
          class="grid grid-cols-[auto_2.75rem_minmax(0,1fr)_8rem_7rem_4rem_auto] items-center gap-3 border-b border-default/50 px-3 py-2 transition last:border-0 hover:bg-white/3"
          :class="{ 'opacity-55': !m.enabled }"
        >
          <!-- toggle -->
          <div class="flex justify-center">
            <USwitch
              :model-value="m.enabled"
              :title="m.enabled ? $t('mods.disable') : $t('mods.enable')"
              @update:model-value="toggle(m, $event)"
            />
          </div>
          <!-- icon -->
          <img v-if="m.icon_url" :src="m.icon_url" class="size-9 rounded-lg object-cover" :alt="m.name ?? m.filename" />
          <div v-else class="flex size-9 items-center justify-center rounded-lg bg-white/5">
            <UIcon :name="kindIcon(m.kind)" class="size-4.5 text-neutral-500" />
          </div>

          <!-- name + provider -->
          <div class="min-w-0">
            <div class="group/n flex items-center gap-1">
              <span class="truncate font-medium" :title="m.filename">{{ m.name ?? m.filename }}</span>
              <UButton
                v-if="m.project_id"
                icon="i-lucide-external-link"
                color="neutral"
                variant="ghost"
                size="xs"
                class="shrink-0 opacity-0 transition group-hover/n:opacity-100"
                :title="$t('mods.openPage')"
                @click="openModPage(m)"
              />
            </div>
            <div class="mt-0.5 flex items-center gap-1.5">
              <span
                class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[10px] font-medium"
                :class="providerBadgeClass(m.provider)"
              >
                {{ $t(`mods.provider.${m.provider}`) }}
              </span>
              <span v-if="kind === 'all'" class="text-[10px] text-neutral-500">{{ $t(kindLabel(m.kind)) }}</span>
            </div>
          </div>

          <!-- version -->
          <span class="truncate font-mono text-xs text-neutral-400" :title="m.version ?? ''">{{ m.version ?? '—' }}</span>

          <!-- updated -->
          <span class="text-xs text-neutral-500">{{ formatDate(m.modified) }}</span>



          <!-- actions -->
          <div class="flex items-center justify-end gap-0.5">
            <UButton
              v-if="!m.project_id && m.kind === 'mod'"
              icon="i-lucide-search"
              color="neutral"
              variant="ghost"
              size="xs"
              :loading="linking"
              :title="$t('mods.checkModrinth')"
              @click="linkLocal"
            />
            <UButton
              v-if="m.project_id && updates[m.project_id]"
              icon="i-lucide-circle-arrow-up"
              color="primary"
              variant="soft"
              size="xs"
              :loading="updatingId === m.filename"
              :title="$t('mods.updateTo', { v: updates[m.project_id]?.version_number ?? '' })"
              @click="updateMod(m)"
            />
            <UButton
              v-if="m.project_id && m.kind === 'mod'"
              icon="i-lucide-replace"
              color="neutral"
              variant="ghost"
              size="xs"
              :title="$t('mods.changeVersion')"
              @click="openVersions(m)"
            />
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              size="xs"
              :title="$t('common.remove')"
              @click="remove(m)"
            />
          </div>
        </div>
      </div>

      <!-- pagination -->
      <div v-if="filtered.length > perPage" class="flex justify-center pt-1">
        <UPagination v-model:page="page" :total="filtered.length" :items-per-page="perPage" />
      </div>
    </template>

    <!-- change-version modal -->
    <UModal v-model:open="versionsOpen" :title="$t('mods.pickVersion')" :ui="{ content: 'max-w-lg' }">
      <template #footer>
        <ModalHint>{{ $t('mods.versionHint') }}</ModalHint>
      </template>
      <template #body>
        <div v-if="loadingVersions" class="py-8 text-center text-sm text-muted">{{ $t('common.loading') }}</div>
        <div v-else-if="!modVersions.length" class="py-8 text-center text-sm text-muted">{{ $t('modrinth.noVersions') }}</div>
        <div v-else class="max-h-[60vh] space-y-1.5 overflow-y-auto">
          <div
            v-for="v in modVersions"
            :key="v.id"
            class="flex items-center gap-3 rounded-lg border border-default bg-white/3 p-2.5"
          >
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="truncate text-sm font-medium">{{ v.name || v.version_number }}</span>
                <UBadge v-if="versionsMod && installedVersionId === v.id" color="success" variant="subtle" size="xs" :label="$t('mods.installed')" />
              </div>
              <div class="mt-0.5 truncate text-[11px] text-neutral-500">
                {{ v.game_versions.slice(0, 4).join(', ') }}<span v-if="v.loaders.length"> · {{ v.loaders.join(', ') }}</span>
              </div>
            </div>
            <UButton
              size="xs"
              color="neutral"
              variant="soft"
              :loading="installingVersionId === v.id"
              :disabled="installedVersionId === v.id || !!installingVersionId"
              :label="$t('mods.useVersion')"
              @click="chooseVersion(v.id)"
            />
          </div>
        </div>
      </template>
    </UModal>

    <!-- delete dependencies modal -->
    <UModal v-model:open="depsOpen" :title="$t('mods.depsTitle', { name: depsTarget?.name ?? depsTarget?.filename ?? '' })" :ui="{ content: 'max-w-md' }">
      <template #body>
        <p class="mb-3 text-sm text-muted">{{ $t('mods.depsDesc') }}</p>
        <div class="max-h-[50vh] space-y-1.5 overflow-y-auto">
          <label
            v-for="d in deps"
            :key="d.filename"
            class="flex cursor-pointer items-center gap-3 rounded-lg border border-default bg-white/3 p-2.5"
          >
            <UCheckbox
              :model-value="depsChecked.has(d.filename)"
              @update:model-value="toggleDep(d.filename, $event)"
            />
            <img v-if="d.icon_url" :src="d.icon_url" class="size-8 shrink-0 rounded-md object-cover" :alt="d.name" >
            <div v-else class="flex size-8 shrink-0 items-center justify-center rounded-md bg-white/5">
              <UIcon name="i-lucide-blocks" class="size-4 text-neutral-500" />
            </div>
            <span class="min-w-0 flex-1 truncate text-sm">{{ d.name }}</span>
          </label>
        </div>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton variant="ghost" color="neutral" :label="$t('mods.depsKeep')" @click="keepDeps" />
          <UButton
            color="error"
            icon="i-lucide-trash-2"
            :label="allDepsChecked ? $t('mods.depsDeleteAll') : $t('mods.depsDeleteSelected')"
            @click="confirmDeps"
          />
        </div>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { openExternal as openUrl } from '~/utils/openExternal'
import type { ModEntry, Instance } from '~/types/launcher'
import type { ModUpdate, ModrinthVersion, ContentKind } from '~/types/modrinth'

/** The kinds that live side by side in this tab, in selector order. */
const KINDS = ['mod', 'resourcepack', 'shader', 'datapack'] as const
type Kind = typeof KINDS[number]
type Entry = ModEntry & { kind: Kind }

const props = defineProps<{ instanceId: string; initialKind?: Kind }>()
const toast = useToast()
const { t } = useI18n()
const browser = useModrinthBrowser()
const modList = useModListModal()
const linkModal = useLinkModsModal()
const blockedModal = useBlockedModsModal()
const instances = useInstancesStore()
const modrinth = useModrinth()
const curseforge = useCurseforge()
const activity = useActivityCenter()

const mods = ref<Entry[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

// --- kind selector ---
const kind = ref<Kind | 'all'>(props.initialKind ?? 'all')

const KIND_META: Record<Kind, { label: string; icon: string }> = {
  mod: { label: 'instance.tabs.mods', icon: 'i-lucide-blocks' },
  resourcepack: { label: 'instance.tabs.resourcepacks', icon: 'i-lucide-image' },
  shader: { label: 'instance.tabs.shaders', icon: 'i-lucide-sparkles' },
  datapack: { label: 'instance.tabs.datapacks', icon: 'i-lucide-package' },
}
const kindIcon = (k: Kind) => KIND_META[k].icon
const kindLabel = (k: Kind) => KIND_META[k].label

const kindTabs = [
  { key: 'all' as const, label: 'content.all', icon: 'i-lucide-layers' },
  ...KINDS.map(k => ({ key: k, label: KIND_META[k].label, icon: KIND_META[k].icon })),
]

const ofKind = computed(() => (kind.value === 'all' ? mods.value : mods.value.filter(m => m.kind === kind.value)))
const counts = computed(() => {
  const out: Record<string, number> = { all: mods.value.length }
  for (const k of KINDS) out[k] = mods.value.filter(m => m.kind === k).length
  return out
})

// What the "Add" button and the empty state target; "all" browses mods.
const browserKind = computed<Kind>(() => (kind.value === 'all' ? 'mod' : kind.value))
const hasMods = computed(() => mods.value.some(m => m.kind === 'mod'))

// Target instance filters (loader + game version) for Modrinth lookups.
const instance = computed(() => instances.instances.find((i: Instance) => i.id === props.instanceId))
const filterLoaders = computed(() => {
  const lt = instance.value?.loader.type
  return lt && lt !== 'vanilla' ? [lt] : undefined
})
const filterGv = computed(() => (instance.value ? [instance.value.mc_version] : undefined))

// --- available updates ---
const updates = ref<Record<string, ModUpdate>>({})
const updatingId = ref<string | null>(null) // filename being updated
const updatingAll = ref(false)
const updateCount = computed(() => mods.value.filter(m => m.project_id && updates.value[m.project_id]).length)

/** Reads every kind off disk and tags each row with the kind it came from. */
async function listAll(): Promise<Entry[]> {
  const lists = await Promise.all(KINDS.map(async k =>
    (await invoke<ModEntry[]>('list_content', { instanceId: props.instanceId, kind: k })).map(m => ({ ...m, kind: k })),
  ))
  return lists.flat()
}

async function refreshUpdates() {
  try {
    const list = await modrinth.checkUpdates(props.instanceId, filterLoaders.value, filterGv.value)
    updates.value = Object.fromEntries(list.map(u => [u.project_id, u]))
  } catch {
    updates.value = {}
  }
}

/** Installs `versionId` for a mod and removes its previous jar. Routes to the
 *  right provider (CurseForge `versionId` is a fileId). */
async function applyVersion(mod: Entry, versionId: string) {
  let added
  if (mod.provider === 'curseforge' && mod.project_id) {
    const res = await curseforge.installWithDeps(props.instanceId, mod.project_id, versionId, filterGv.value?.[0], filterLoaders.value?.[0])
    added = res.added
    for (const b of res.blocked) {
      toast.add({
        title: t('modrinth.blocked', { name: b.name }),
        color: 'warning',
        actions: [{ label: t('logs.openLink'), onClick: () => openUrl(b.url) }],
      })
    }
  } else {
    added = await modrinth.installWithDeps(props.instanceId, versionId, filterGv.value?.[0], filterLoaders.value?.[0])
  }
  const fresh = added.find(a => a.project_id === mod.project_id)
  if (fresh && fresh.filename !== mod.filename) {
    await invoke('delete_mod', { instanceId: props.instanceId, filename: mod.filename })
  }
}

async function updateMod(mod: Entry) {
  const u = mod.project_id ? updates.value[mod.project_id] : null
  if (!u) return
  updatingId.value = mod.filename
  const tid = activity.startTask(t('activity.updatingMod', { name: mod.name ?? mod.filename }))
  try {
    await applyVersion(mod, u.version_id)
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(tid)
    updatingId.value = null
  }
}

async function updateAll() {
  updatingAll.value = true
  const tid = activity.startTask(t('activity.updatingMods'))
  try {
    // Both providers update via bulk backend calls (no per-mod requests → no 429).
    await modrinth.updateAll(props.instanceId, filterLoaders.value, filterGv.value)
    await curseforge.updateAll(props.instanceId, filterLoaders.value, filterGv.value)
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(tid)
    updatingAll.value = false
  }
}

// --- change version modal ---
const versionsMod = ref<Entry | null>(null)
const versionsOpen = computed({
  get: () => versionsMod.value !== null,
  set: (v: boolean) => { if (!v) versionsMod.value = null },
})
const modVersions = ref<ModrinthVersion[]>([])
const loadingVersions = ref(false)
const installingVersionId = ref<string | null>(null)
const installedVersionId = computed(() => versionsMod.value?.version_id ?? null)

async function openVersions(mod: Entry) {
  if (!mod.project_id) return
  versionsMod.value = mod
  modVersions.value = []
  loadingVersions.value = true
  try {
    modVersions.value = mod.provider === 'curseforge'
      ? await curseforge.versions(mod.project_id, filterLoaders.value, filterGv.value)
      : await modrinth.versions(mod.project_id, filterLoaders.value, filterGv.value)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    loadingVersions.value = false
  }
}

async function chooseVersion(versionId: string) {
  const mod = versionsMod.value
  if (!mod) return
  installingVersionId.value = versionId
  const tid = activity.startTask(t('activity.changingVersion', { name: mod.name ?? mod.filename }))
  try {
    await applyVersion(mod, versionId)
    versionsMod.value = null
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    activity.endTask(tid)
    installingVersionId.value = null
  }
}

/** Opens a mod's page in the external browser. */
function openModPage(mod: Entry) {
  if (!mod.project_id) return
  if (mod.provider === 'curseforge') {
    openUrl(`https://www.curseforge.com/projects/${mod.project_id}`)
  } else {
    openUrl(`https://modrinth.com/project/${mod.project_id}`)
  }
}

// --- controls ---
type SortCol = 'name' | 'version' | 'updated' | 'state' | 'update'
const search = ref('')
const perPage = ref(25)
const page = ref(1)

// Column-header sorting: click cycles asc → desc → off. Defaults to floating
// mods with an available update to the top.
const sortCol = ref<SortCol | null>('update')
const sortDir = ref<'asc' | 'desc'>('asc')

/** Whether a mod has an available update. */
const hasUpdate = (m: Entry) => !!(m.project_id && updates.value[m.project_id])

function toggleSort(col: SortCol) {
  if (sortCol.value !== col) {
    sortCol.value = col
    sortDir.value = 'asc'
  } else if (sortDir.value === 'asc') {
    sortDir.value = 'desc'
  } else {
    sortCol.value = null // off
  }
}
const sortIcon = (col: SortCol) =>
  sortCol.value === col ? (sortDir.value === 'asc' ? 'i-lucide-arrow-up' : 'i-lucide-arrow-down') : 'i-lucide-chevrons-up-down'

const perPageItems = [10, 25, 50, 100].map(n => ({ label: t('mods.perPage', { n }), value: n }))

const nameOf = (m: Entry) => (m.name ?? m.filename).toLowerCase()
const formatDate = (ms: number) => (ms ? new Date(ms).toLocaleDateString() : '—')

const providerBadgeClass = (provider: string) => {
  if (provider === 'curseforge') return 'bg-orange-500/10 text-orange-400 ring-1 ring-inset ring-orange-500/25'
  if (provider === 'modrinth') return 'bg-green-500/10 text-green-400 ring-1 ring-inset ring-green-500/25'
  return 'bg-neutral-500/10 text-neutral-400 ring-1 ring-inset ring-neutral-500/25'
}

// Ascending comparator per column; direction is applied afterwards.
function compare(a: Entry, b: Entry, col: SortCol): number {
  switch (col) {
    case 'name': return nameOf(a).localeCompare(nameOf(b))
    case 'version': return (a.version ?? '').localeCompare(b.version ?? '', undefined, { numeric: true })
    case 'updated': return a.modified - b.modified
    case 'state': return Number(b.enabled) - Number(a.enabled) // enabled first
    case 'update': return Number(hasUpdate(b)) - Number(hasUpdate(a)) // updatable first
  }
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase()
  let list = ofKind.value
  if (q) list = list.filter(m => nameOf(m).includes(q) || m.filename.toLowerCase().includes(q))

  const col = sortCol.value
  if (!col) return list
  const dir = sortDir.value === 'asc' ? 1 : -1
  return [...list].sort((a, b) => compare(a, b, col) * dir || nameOf(a).localeCompare(nameOf(b)))
})

const paged = computed(() => {
  const start = (page.value - 1) * perPage.value
  return filtered.value.slice(start, start + perPage.value)
})

// Reset to page 1 when the view changes; clamp if the list shrinks.
watch([search, sortCol, sortDir, perPage, kind], () => { page.value = 1 })
watch(filtered, () => {
  const max = Math.max(1, Math.ceil(filtered.value.length / perPage.value))
  if (page.value > max) page.value = max
})

async function load() {
  loading.value = true
  error.value = null
  try {
    mods.value = await listAll()
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
  void linkAndCheck()
  void loadBlocked()
  void loadConflicts()
}

// Whether a CurseForge API key is configured (enables CF linking).
const cfEnabled = ref(false)
curseforge.enabled().then(v => (cfEnabled.value = v)).catch(() => {})

// Likely mod conflicts (wrong loader, duplicates).
const conflicts = ref<{ filename: string, name: string, kind: string, detail: string }[]>([])
async function loadConflicts() {
  try { conflicts.value = await invoke('check_conflicts', { instanceId: props.instanceId }) }
  catch { conflicts.value = [] }
}

// CurseForge mods blocked from auto-download, awaiting manual fetch.
const blockedCount = ref(0)
async function loadBlocked() {
  try { blockedCount.value = (await curseforge.getBlocked(props.instanceId)).length }
  catch { blockedCount.value = 0 }
}
function openBlocked() {
  blockedModal.open(props.instanceId, () => { load(); loadBlocked() })
}

// Auto-link is a quiet best-effort Modrinth pass on load; the manual button
// opens the per-file provider-choice dialog. Then check for updates.
async function linkAndCheck() {
  try {
    const matched = await modrinth.matchLocal(props.instanceId)
    if (matched > 0) {
      mods.value = await listAll()
    }
  } catch { /* offline / not found — ignore on auto-run */ }
  refreshUpdates()
}

const hasLocal = computed(() => mods.value.some(m => m.kind === 'mod' && !m.project_id))
const linking = ref(false)

/** Manual: open the per-file provider-choice dialog for unmatched local jars. */
function linkLocal() {
  const files = mods.value.filter(m => m.kind === 'mod' && !m.project_id).map(m => m.filename)
  if (!files.length) {
    toast.add({ title: t('mods.noMatch'), color: 'neutral' })
    return
  }
  linkModal.open({
    instanceId: props.instanceId,
    files,
    cfEnabled: cfEnabled.value,
    onDone: async () => {
      mods.value = await listAll()
      refreshUpdates()
    },
  })
}

async function toggle(mod: Entry, enabled: boolean) {
  try {
    // Mods keep their own command (it knows the mods/ folder); everything else
    // goes through content.rs, which validates names and handles folder packs.
    if (mod.kind === 'mod') {
      await invoke('set_mod_enabled', { instanceId: props.instanceId, filename: mod.filename, enabled })
    } else {
      await invoke('set_content_enabled', { id: props.instanceId, kind: mod.kind, filename: mod.filename, enabled })
    }
    mod.enabled = enabled
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

// --- delete (with orphaned-dependency prompt) ---
interface RemovableDep { project_id: string; name: string; filename: string; icon_url: string | null; kind: string }
const depsOpen = ref(false)
const depsTarget = ref<Entry | null>(null)
const deps = ref<RemovableDep[]>([])
const depsChecked = ref<Set<string>>(new Set())
const allDepsChecked = computed(() => deps.value.length > 0 && depsChecked.value.size === deps.value.length)

function toggleDep(filename: string, on: boolean) {
  const next = new Set(depsChecked.value)
  if (on) next.add(filename)
  else next.delete(filename)
  depsChecked.value = next
}

/** Deletes the mod and the given dependency filenames, updating the list. */
async function deleteMods(mod: Entry, depFilenames: string[]) {
  try {
    const all = [mod.filename, ...depFilenames]
    for (const filename of all) {
      await invoke('delete_mod', { instanceId: props.instanceId, filename })
    }
    const removed = new Set(all)
    mods.value = mods.value.filter(m => !removed.has(m.filename))
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function remove(mod: Entry) {
  // Only mods carry dependencies worth asking about.
  if (mod.kind !== 'mod') {
    try {
      await invoke('delete_content', { id: props.instanceId, kind: mod.kind, filename: mod.filename })
      mods.value = mods.value.filter(m => m !== mod)
    } catch (e) {
      toast.add({ title: String(e), color: 'error' })
    }
    return
  }
  try {
    const found = await invoke<RemovableDep[]>('get_removable_dependencies', {
      instanceId: props.instanceId,
      filename: mod.filename,
    })
    if (!found.length) {
      await deleteMods(mod, [])
      return
    }
    depsTarget.value = mod
    deps.value = found
    depsChecked.value = new Set(found.map(d => d.filename))
    depsOpen.value = true
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function confirmDeps() {
  const mod = depsTarget.value
  if (!mod) return
  const selected = deps.value.filter(d => depsChecked.value.has(d.filename)).map(d => d.filename)
  depsOpen.value = false
  await deleteMods(mod, selected)
}

async function keepDeps() {
  const mod = depsTarget.value
  if (!mod) return
  depsOpen.value = false
  await deleteMods(mod, [])
}

function openBrowser() {
  const instance = instances.instances.find((i: Instance) => i.id === props.instanceId)
  browser.open({
    kind: browserKind.value as ContentKind,
    mode: 'install',
    instanceId: props.instanceId,
    gameVersion: instance?.mc_version,
    loader: instance?.loader.type,
    onInstalled: () => load(),
  })
}

watch(() => props.instanceId, load, { immediate: true })
</script>
