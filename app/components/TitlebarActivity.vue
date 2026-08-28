<template>
  <div class="flex items-center gap-2">
    <button
      v-if="updater.available.value && updater.status.value !== 'downloading' && updater.status.value !== 'ready'"
      type="button"
      style="-webkit-app-region: no-drag"
      class="flex h-7 items-center gap-1.5 rounded-full border border-primary-500/40 bg-primary-500/15 px-3 text-xs font-medium text-primary-300 transition hover:border-primary-500/70 hover:bg-primary-500/25"
      :title="$t('update.available', { version: updater.newVersion.value })"
      @click="updater.downloadAndInstall()"
    >
      <UIcon name="i-lucide-download" class="size-3.5 shrink-0" />
      <span class="shrink-0">{{ $t('update.titlebarReady', { version: updater.newVersion.value }) }}</span>
    </button>

    <component
      :is="clickable ? 'button' : 'div'"
      :type="clickable ? 'button' : undefined"
      style="-webkit-app-region: no-drag"
      class="flex h-7 items-center gap-2 rounded-full border border-white/8 bg-white/4 px-3 text-xs transition"
      :class="clickable ? 'cursor-pointer hover:border-white/20 hover:bg-white/8' : ''"
      :title="clickable ? $t('activity.openLogs') : view.label"
      @click="onClick"
    >
      <template v-if="view.kind === 'install'">
        <UIcon name="i-lucide-loader-circle" class="size-3.5 shrink-0 animate-spin text-primary-400" />
        <span class="max-w-48 truncate text-neutral-200">{{ view.label }}</span>
        <span v-if="view.percent !== null" class="shrink-0 font-mono text-neutral-400">{{ view.percent }}%</span>
        <span class="relative h-1 w-14 shrink-0 overflow-hidden rounded-full bg-white/10">
          <span
            class="absolute inset-y-0 left-0 rounded-full bg-primary-500 transition-[width] duration-200"
            :style="{ width: (view.percent ?? 0) + '%' }"
          />
        </span>
      </template>

      <template v-else-if="view.kind === 'task'">
        <UIcon name="i-lucide-loader-circle" class="size-3.5 shrink-0 animate-spin text-primary-400" />
        <span class="max-w-64 truncate text-neutral-200">{{ view.label }}</span>
      </template>

      <template v-else-if="view.kind === 'running'">
        <span class="size-2 shrink-0 rounded-full bg-[#3fb877] shadow-[0_0_8px_#3fb877]" />
        <span class="max-w-56 truncate text-neutral-200">{{ view.label }}</span>
      </template>

      <template v-else>
        <span class="size-2 shrink-0 rounded-full bg-neutral-600" />
        <span class="text-neutral-400">{{ view.label }}</span>
      </template>

      <UIcon v-if="clickable" name="i-lucide-scroll-text" class="size-3.5 shrink-0 text-neutral-500" />
    </component>
  </div>
</template>

<script setup lang="ts">
const ac = useActivityCenter()
const instances = useInstancesStore()
const updater = useAutoUpdate()
const { t } = useI18n()

const nameFor = (id: string) =>
  instances.instances.find(i => i.id === id)?.name ?? t('activity.unknownInstance')

interface View {
  kind: 'idle' | 'install' | 'task' | 'running'
  label: string
  percent: number | null
}

const view = computed<View>(() => {
  const top = ac.top.value
  const labels = ac.taskLabels.value

  if (updater.status.value === 'downloading' || updater.status.value === 'ready') {
    const label = updater.status.value === 'ready' ? t('update.ready') : t('update.titlebar')
    return { kind: 'install', label, percent: updater.status.value === 'ready' ? 100 : updater.progress.value }
  }
  const mp = ac.modpack.value
  if (mp) {
    const percent = mp.total > 0 ? Math.min(100, Math.round((mp.current / mp.total) * 100)) : null
    return { kind: 'install', label: t('activity.downloadingPack', { name: mp.name }), percent }
  }
  if (top?.kind === 'install') {
    const percent = top.total > 0 ? Math.min(100, Math.round((top.current / top.total) * 100)) : null
    return { kind: 'install', label: t('activity.downloading', { name: nameFor(top.instanceId) }), percent }
  }
  if (labels.length) {
    const label = labels.length > 1 ? t('activity.tasks', { n: labels.length }) : labels[0]!
    return { kind: 'task', label, percent: null }
  }
  if (top?.kind === 'running') {
    return { kind: 'running', label: t('activity.running', { name: nameFor(top.instanceId) }), percent: null }
  }
  return { kind: 'idle', label: t('activity.idle'), percent: null }
})

const runningInstance = computed(() => ac.list.value.find(a => a.kind === 'running')?.instanceId ?? null)
const clickable = computed(() => runningInstance.value !== null)

function onClick() {
  if (!clickable.value) return
  ac.openLiveLogs(runningInstance.value ?? undefined)
}
</script>
