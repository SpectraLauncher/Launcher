<template>
  <div class="flex h-full flex-col gap-3 p-4">
    <div class="flex flex-wrap items-center gap-2">
      <USelectMenu
        v-if="options.length > 1"
        v-model="selectedValue"
        :items="options"
        value-key="value"
        class="w-56"
        size="sm"
      />
      <span v-else class="text-sm font-medium">{{ nameOf(selected) }}</span>
      <span class="flex items-center gap-1.5 text-xs text-muted">
        <span
          class="size-2 rounded-full"
          :class="isRunning ? 'bg-[#3fb877] shadow-[0_0_8px_#3fb877]' : 'bg-neutral-600'"
        />
        {{ isRunning ? $t('logs.live') : $t('activity.idle') }} · {{ lines.length }}
      </span>
      <div class="ml-auto flex items-center gap-1.5">
        <USwitch v-model="autoClose" size="xs" :label="$t('logs.autoClose')" class="mr-2" />
        <UButton
          icon="i-lucide-arrow-down-to-line"
          color="neutral"
          variant="ghost"
          size="xs"
          :class="autoscroll ? 'text-primary-400' : ''"
          :title="$t('logs.autoscroll')"
          square
          @click="toggleAutoscroll"
        />
        <UButton
          icon="i-lucide-copy"
          color="neutral"
          variant="ghost"
          size="xs"
          :title="$t('common.copy')"
          square
          @click="copyAll"
        />
        <UButton
          icon="i-lucide-eraser"
          color="neutral"
          variant="ghost"
          size="xs"
          :title="$t('logs.clear')"
          square
          @click="clear"
        />
      </div>
    </div>

    <div
      ref="scroller"
      class="min-h-0 flex-1 overflow-auto rounded-xl border border-default bg-black/40 px-3 py-2 font-mono text-[11px] leading-relaxed"
      @scroll="onScroll"
    >
      <div v-if="!lines.length" class="py-16 text-center text-sm text-muted">{{ $t('logs.waiting') }}</div>
      <p
        v-for="(l, i) in colored"
        :key="i"
        class="whitespace-pre-wrap break-words"
        :class="l.cls"
      >{{ l.text }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

definePageMeta({ layout: 'browser' })

interface ConsoleChunk {
  lines: string[]
  cursor: number
  reset: boolean
}

const KEEP = 5000
const AUTO_CLOSE_KEY = 'spectra-console-autoclose'

const route = useRoute()
const instances = useInstancesStore()
const { t } = useI18n()
const toast = useToast()

const selected = ref<string | null>(typeof route.query.instance === 'string' ? route.query.instance : null)
const running = ref<string[]>([])
const lines = ref<string[]>([])
let cursor = 0

const selectedValue = computed({
  get: () => selected.value ?? undefined,
  set: (value: string | undefined) => { selected.value = value ?? null },
})

const nameOf = (id: string | null) =>
  (id ? instances.instances.find(i => i.id === id)?.name : null) ?? t('activity.unknownInstance')

const options = computed(() => {
  const ids = [...new Set([...(selected.value ? [selected.value] : []), ...running.value])]
  return ids.map(value => ({ value, label: nameOf(value) }))
})

const isRunning = computed(() => !!selected.value && running.value.includes(selected.value))

const autoClose = ref((() => {
  try {
    return localStorage.getItem(AUTO_CLOSE_KEY) === 'true'
  } catch {
    return false
  }
})())
watch(autoClose, (value) => {
  try {
    localStorage.setItem(AUTO_CLOSE_KEY, String(value))
  } catch {
  }
})

async function onExit(id: string) {
  await refreshRunning()
  if (!autoClose.value || id !== selected.value) return
  const next = running.value.find(other => other !== id)
  if (next) {
    selected.value = next
    return
  }
  await getCurrentWindow().close()
}

async function pump() {
  if (!selected.value) return
  const id = selected.value
  let chunk: ConsoleChunk
  try {
    chunk = await invoke<ConsoleChunk>('read_console', { id, cursor })
  } catch {
    return
  }
  if (id !== selected.value) return
  cursor = chunk.cursor
  if (!chunk.lines.length && !chunk.reset) return
  const next = chunk.reset ? [] : lines.value.slice()
  next.push(...chunk.lines)
  if (next.length > KEEP) next.splice(0, next.length - KEEP)
  lines.value = next
}

async function refreshRunning() {
  try {
    running.value = await invoke<string[]>('running_instances')
  } catch {  }
  if (!selected.value && running.value[0]) selected.value = running.value[0]
}

watch(selected, () => {
  cursor = 0
  lines.value = []
  void pump()
})

function levelClass(line: string): string {
  const m = line.match(/\b(FATAL|ERROR|SEVERE|WARN(?:ING)?|INFO|DEBUG|TRACE)\b/)
  switch (m?.[1]) {
    case 'FATAL':
    case 'ERROR':
    case 'SEVERE':
      return 'text-red-400'
    case 'WARN':
    case 'WARNING':
      return 'text-amber-400'
    case 'DEBUG':
    case 'TRACE':
      return 'text-neutral-500'
    case 'INFO':
      return 'text-neutral-300'
  }
  if (/^\s*at\s|Exception|Caused by:|\bError\b/.test(line)) return 'text-red-400/80'
  return 'text-neutral-400'
}

const colored = computed(() => lines.value.map(text => ({ text, cls: levelClass(text) })))

const scroller = ref<HTMLElement | null>(null)
const autoscroll = ref(true)

function scrollToBottom() {
  const el = scroller.value
  if (el) el.scrollTop = el.scrollHeight
}
function toggleAutoscroll() {
  autoscroll.value = !autoscroll.value
  if (autoscroll.value) nextTick(scrollToBottom)
}
function onScroll() {
  const el = scroller.value
  if (!el) return
  autoscroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 40
}

watch(() => lines.value.length, () => {
  if (autoscroll.value) nextTick(scrollToBottom)
})

async function copyAll() {
  try {
    await navigator.clipboard.writeText(lines.value.join('\n'))
    toast.add({ title: t('common.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

function clear() {
  lines.value = []
  if (selected.value) void invoke('clear_console', { id: selected.value }).catch(() => {})
}

let pumpTimer: ReturnType<typeof setInterval> | undefined
let runningTimer: ReturnType<typeof setInterval> | undefined
let unlisten: UnlistenFn | null = null
let unlistenExit: UnlistenFn | null = null

onMounted(async () => {
  if (!instances.instances.length) instances.load()
  await refreshRunning()
  void pump()
  pumpTimer = setInterval(pump, 500)
  runningTimer = setInterval(refreshRunning, 2000)
  unlisten = await listen<string>('console://select', (e) => { selected.value = e.payload })
  unlistenExit = await listen<{ instance_id: string }>('mc://exited', (e) => { void onExit(e.payload.instance_id) })
})

onBeforeUnmount(() => {
  clearInterval(pumpTimer)
  clearInterval(runningTimer)
  unlisten?.()
  unlistenExit?.()
})
</script>
