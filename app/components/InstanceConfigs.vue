<template>
  <div class="flex min-h-[60vh] gap-4">
    <aside class="w-72 shrink-0">
      <UInput
        v-model="fileQuery"
        icon="i-lucide-search"
        size="sm"
        :placeholder="$t('configs.searchFiles')"
        class="mb-2 w-full"
      />
      <div v-if="loadingList" class="space-y-1.5">
        <div v-for="n in 8" :key="n" class="h-7 animate-pulse rounded-lg bg-white/5" />
      </div>
      <p v-else-if="!files.length" class="px-2 py-4 text-sm text-muted">{{ $t('configs.empty') }}</p>
      <div v-else class="max-h-[68vh] space-y-3 overflow-y-auto pr-1">
        <div v-for="group in groups" :key="group.key">
          <p class="mb-0.5 px-2 text-[11px] font-semibold uppercase tracking-wide text-dimmed">{{ group.key }}</p>
          <button
            v-for="file in group.files"
            :key="file.rel"
            type="button"
            class="flex w-full items-center gap-2 rounded-lg px-2 py-1 text-left text-sm transition"
            :class="current?.rel === file.rel ? 'bg-primary-500/15 text-primary-300' : 'text-neutral-300 hover:bg-white/5'"
            @click="open(file)"
          >
            <span class="min-w-0 flex-1 truncate">{{ label(file.rel) }}</span>
            <span class="shrink-0 text-[10px] uppercase text-dimmed">{{ file.format }}</span>
          </button>
        </div>
      </div>
    </aside>

    <section class="min-w-0 flex-1">
      <p v-if="!current" class="py-16 text-center text-sm text-muted">{{ $t('configs.pick') }}</p>

      <template v-else>
        <div class="mb-3 flex flex-wrap items-center gap-2">
          <p class="min-w-0 flex-1 truncate font-mono text-sm">{{ current.rel }}</p>
          <UInput
            v-if="entries.length"
            v-model="entryQuery"
            icon="i-lucide-search"
            size="sm"
            :placeholder="$t('configs.searchSettings')"
            class="w-56"
          />
          <UButton
            size="sm"
            color="neutral"
            variant="ghost"
            icon="i-lucide-folder-search"
            :label="$t('configs.reveal')"
            @click="reveal"
          />
        </div>

        <UAlert
          v-if="running"
          color="warning"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          class="mb-3"
          :description="$t('configs.running')"
        />

        <div v-if="loadingDoc" class="space-y-2">
          <div v-for="n in 6" :key="n" class="h-12 animate-pulse rounded-lg bg-white/5" />
        </div>

        <p v-else-if="docError" class="py-10 text-center text-sm text-muted">
          {{ $t('configs.unreadable', { reason: docError }) }}
        </p>

        <p v-else-if="!visible.length" class="py-10 text-center text-sm text-muted">{{ $t('configs.noMatch') }}</p>

        <div v-else class="max-h-[64vh] space-y-0.5 overflow-y-auto pr-1 pb-16">
          <template v-for="entry in visible" :key="keyOf(entry)">
            <div
              v-if="entry.kind === 'section'"
              class="pb-1 pt-4"
              :style="{ paddingLeft: `${indent(entry)}px` }"
            >
              <p class="text-sm font-semibold text-highlighted">{{ nameOf(entry) }}</p>
              <p v-if="entry.comment" class="whitespace-pre-line text-xs text-dimmed">{{ entry.comment }}</p>
            </div>

            <div
              v-else
              class="flex flex-wrap items-start gap-x-4 gap-y-2 rounded-lg py-2 pr-2 transition hover:bg-white/3"
              :style="{ paddingLeft: `${indent(entry) + 8}px` }"
            >
              <div class="min-w-48 flex-1">
                <p class="break-words text-sm font-medium">
                  {{ nameOf(entry) }}
                  <span v-if="edits.has(keyOf(entry))" class="text-primary-400">•</span>
                </p>
                <p v-if="entry.comment" class="mt-0.5 whitespace-pre-line text-xs text-muted">{{ entry.comment }}</p>
                <p v-if="entry.default !== null" class="mt-0.5 text-[11px] text-dimmed">
                  {{ $t('configs.default', { value: shown(entry.default) }) }}
                </p>
              </div>

              <div class="flex shrink-0 items-center gap-2">
                <USwitch
                  v-if="entry.kind === 'bool'"
                  :model-value="valueOf(entry) as boolean"
                  :disabled="running"
                  @update:model-value="set(entry, $event)"
                />

                <USelect
                  v-else-if="entry.kind === 'string' && entry.options.length"
                  :model-value="valueOf(entry) as string"
                  :items="entry.options"
                  :disabled="running"
                  class="w-52"
                  @update:model-value="set(entry, $event)"
                />

                <template v-else-if="entry.kind === 'int' || entry.kind === 'float'">
                  <USlider
                    v-if="sliderFor(entry)"
                    :model-value="valueOf(entry) as number"
                    :min="entry.min!"
                    :max="entry.max!"
                    :step="stepOf(entry)"
                    :disabled="running"
                    class="w-40"
                    @update:model-value="set(entry, numberFor(entry, $event))"
                  />
                  <UInputNumber
                    :model-value="valueOf(entry) as number"
                    :min="entry.min ?? undefined"
                    :max="entry.max ?? undefined"
                    :step="stepOf(entry)"
                    :format-options="{ maximumFractionDigits: 10, useGrouping: false }"
                    :disabled="running"
                    class="w-36"
                    @update:model-value="set(entry, numberFor(entry, $event))"
                  />
                </template>

                <UInput
                  v-else-if="entry.kind === 'string'"
                  :model-value="valueOf(entry) as string"
                  :disabled="running"
                  class="w-64"
                  @update:model-value="set(entry, String($event))"
                />

                <UInputTags
                  v-else-if="entry.kind === 'list'"
                  :model-value="(valueOf(entry) as unknown[]).map(String)"
                  :disabled="running"
                  class="w-72"
                  @update:model-value="set(entry, listFor(entry, $event))"
                />

                <span v-else class="text-xs text-dimmed">{{ $t('configs.editInFile') }}</span>

                <UButton
                  v-if="resettable(entry)"
                  size="xs"
                  color="neutral"
                  variant="ghost"
                  icon="i-lucide-rotate-ccw"
                  :disabled="running"
                  :title="$t('configs.reset')"
                  :aria-label="$t('configs.reset')"
                  @click="set(entry, entry.default)"
                />
              </div>
            </div>
          </template>
        </div>

        <div
          v-if="edits.size"
          class="sticky bottom-0 mt-2 flex items-center gap-2 rounded-xl border border-default bg-elevated/95 px-4 py-2 backdrop-blur"
        >
          <span class="flex-1 text-sm text-muted">{{ $t('configs.changes', { n: edits.size }, edits.size) }}</span>
          <UButton color="neutral" variant="ghost" :label="$t('configs.discard')" @click="edits.clear()" />
          <UButton icon="i-lucide-save" :loading="saving" :disabled="running" :label="$t('configs.save')" @click="save" />
        </div>
      </template>
    </section>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { ask } from '@tauri-apps/plugin-dialog'

type Kind = 'section' | 'bool' | 'int' | 'float' | 'string' | 'list' | 'other'

interface ConfigFile {
  rel: string
  path: string
  format: string
  size: number
}

interface Entry {
  path: string[]
  kind: Kind
  value: unknown
  comment: string | null
  default: unknown
  min: number | null
  max: number | null
  options: string[]
}

const props = defineProps<{ instanceId: string }>()

const { t } = useI18n()
const toast = useToast()
const mc = useMinecraftLaunch(() => props.instanceId)
const running = computed(() => mc.stage.value !== 'idle')

const files = ref<ConfigFile[]>([])
const loadingList = ref(true)
const fileQuery = ref('')
const current = ref<ConfigFile | null>(null)
const entries = ref<Entry[]>([])
const loadingDoc = ref(false)
const docError = ref('')
const entryQuery = ref('')
const edits = reactive(new Map<string, unknown>())
const saving = ref(false)

const label = (rel: string) => rel.replace(/^config\//, '')

function groupOf(rel: string): string {
  if (rel === 'options.txt') return 'Minecraft'
  const parts = rel.split('/')
  if (parts[0] === 'saves') return `${parts[1]} · serverconfig`
  const inner = parts.slice(1)
  if (inner.length > 1) return inner[0]!.toLowerCase()
  const stem = inner[0]!.replace(/\.[^.]+$/, '')
  return (stem.split(/[-_.]/)[0] || stem).toLowerCase()
}

const groups = computed(() => {
  const query = fileQuery.value.trim().toLowerCase()
  const map = new Map<string, ConfigFile[]>()
  for (const file of files.value) {
    if (query && !file.rel.toLowerCase().includes(query)) continue
    const key = groupOf(file.rel)
    map.set(key, [...(map.get(key) ?? []), file])
  }
  return [...map.entries()]
    .sort(([a], [b]) => (a === 'Minecraft' ? -1 : b === 'Minecraft' ? 1 : a.localeCompare(b)))
    .map(([key, list]) => ({ key, files: list }))
})

const keyOf = (entry: Entry) => JSON.stringify(entry.path)
const nameOf = (entry: Entry) => {
  const last = entry.path[entry.path.length - 1] ?? ''
  return /^\d+$/.test(last) ? `[${last}]` : last
}
const indent = (entry: Entry) => Math.max(0, entry.path.length - 1) * 14

const visible = computed(() => {
  const query = entryQuery.value.trim().toLowerCase()
  if (!query) return entries.value
  return entries.value.filter(e => e.kind !== 'section'
    && (e.path.join('.').toLowerCase().includes(query) || (e.comment ?? '').toLowerCase().includes(query)))
})

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)
const valueOf = (entry: Entry) => (edits.has(keyOf(entry)) ? edits.get(keyOf(entry)) : entry.value)
const shown = (value: unknown) => (typeof value === 'string' ? value : JSON.stringify(value))

function set(entry: Entry, value: unknown) {
  if (value === undefined || value === null) return
  if (same(value, entry.value)) edits.delete(keyOf(entry))
  else edits.set(keyOf(entry), value)
}

const resettable = (entry: Entry) => entry.default !== null && !same(valueOf(entry), entry.default)

const sliderFor = (entry: Entry) =>
  entry.min !== null && entry.max !== null && entry.max > entry.min && entry.max - entry.min <= 100_000

const stepOf = (entry: Entry) => {
  if (entry.kind === 'int') return 1
  if (entry.min !== null && entry.max !== null && entry.max > entry.min) return (entry.max - entry.min) / 100
  return 0.1
}

function numberFor(entry: Entry, value: number | number[] | null | undefined): number | undefined {
  const n = Array.isArray(value) ? value[0] : value
  if (typeof n !== 'number' || Number.isNaN(n)) return undefined
  return entry.kind === 'int' ? Math.round(n) : n
}

function listFor(entry: Entry, items: unknown[]): unknown[] {
  const numeric = Array.isArray(entry.value) && entry.value.length > 0 && entry.value.every(v => typeof v === 'number')
  return items.map(item => (numeric && item !== '' && !Number.isNaN(Number(item)) ? Number(item) : String(item)))
}

async function loadList() {
  loadingList.value = true
  try {
    files.value = await invoke<ConfigFile[]>('list_configs', { id: props.instanceId })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    loadingList.value = false
  }
}

async function loadDoc(file: ConfigFile) {
  loadingDoc.value = true
  docError.value = ''
  entries.value = []
  try {
    const doc = await invoke<{ entries: Entry[] }>('read_config', { id: props.instanceId, rel: file.rel })
    if (current.value?.rel === file.rel) entries.value = doc.entries
  } catch (e) {
    if (current.value?.rel === file.rel) docError.value = errorText(e)
  } finally {
    loadingDoc.value = false
  }
}

async function open(file: ConfigFile) {
  if (current.value?.rel === file.rel) return
  if (edits.size && !(await ask(t('configs.discardAsk', { file: label(current.value?.rel ?? '') }), { kind: 'warning' }))) return
  edits.clear()
  entryQuery.value = ''
  current.value = file
  await loadDoc(file)
}

async function save() {
  if (!current.value || !edits.size) return
  saving.value = true
  try {
    const changes = [...edits.entries()].map(([key, value]) => ({ path: JSON.parse(key) as string[], value }))
    await invoke('write_config', { id: props.instanceId, rel: current.value.rel, changes })
    edits.clear()
    toast.add({ title: t('configs.saved'), color: 'success' })
    await loadDoc(current.value)
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    saving.value = false
  }
}

function reveal() {
  if (current.value) void invoke('reveal_in_explorer', { path: current.value.path }).catch(() => {})
}

watch(() => props.instanceId, () => {
  current.value = null
  entries.value = []
  edits.clear()
  void loadList()
})

onMounted(loadList)
</script>
