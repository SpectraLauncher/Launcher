<template>
  <UModal v-model:open="open" :title="title" :ui="{ content: 'max-w-5xl' }">
    <template #body>
      <div class="space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <USelectMenu
            v-if="runningOptions.length > 1"
            v-model="selected"
            :items="runningOptions"
            value-key="value"
            class="w-56"
            size="sm"
          />
          <span class="flex items-center gap-1.5 text-xs text-muted">
            <span class="size-2 rounded-full bg-[#3fb877] shadow-[0_0_8px_#3fb877]" />
            {{ $t('logs.live') }} · {{ lines.length }}
          </span>
          <div class="ml-auto flex items-center gap-1.5">
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
          class="h-[64vh] overflow-auto rounded-xl border border-default bg-black/40 px-3 py-2 font-mono text-[11px] leading-relaxed"
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
  </UModal>
</template>

<script setup lang="ts">
const ac = useActivityCenter()
const instances = useInstancesStore()
const { t } = useI18n()
const toast = useToast()

const open = ac.liveLogsOpen
const selected = ac.liveLogsInstance

const runningOptions = computed(() =>
  ac.list.value
    .filter(a => a.kind === 'running')
    .map(a => ({ value: a.instanceId, label: instances.instances.find(i => i.id === a.instanceId)?.name ?? a.instanceId })),
)

watch([open, runningOptions], () => {
  if (!open.value) return
  if (!selected.value || !runningOptions.value.some(o => o.value === selected.value)) {
    selected.value = runningOptions.value[0]?.value ?? null
  }
}, { immediate: true })

const title = computed(() => {
  const name = instances.instances.find(i => i.id === selected.value)?.name
  return name ? `${t('logs.live')} — ${name}` : t('logs.live')
})

const lines = computed(() => (selected.value ? ac.logsFor(selected.value).value : []))

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
watch(open, (v) => {
  if (v) nextTick(scrollToBottom)
})

async function copyAll() {
  try {
    await navigator.clipboard.writeText(lines.value.join('\n'))
    toast.add({ title: t('common.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}
function clear() {
  if (selected.value) ac.clearLog(selected.value)
}
</script>
