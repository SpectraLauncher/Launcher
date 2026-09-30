<template>
  <div class="mb-6 rounded-[15px] border border-primary-500/25 bg-linear-[160deg] from-primary-500/15 to-primary-500/5 p-[13px]">
    <div class="mb-[9px] flex items-center gap-1.5 text-[10px] font-semibold tracking-[0.12em] text-primary-300">
      <span v-if="playing" class="size-1.5 rounded-full bg-[#3fb877] shadow-[0_0_6px_#3fb877]" />
      {{ playing ? $t('instanceCard.nowPlaying') : $t('instanceCard.title') }}
    </div>

    <template v-if="selected">
      <div class="mb-3 flex items-center gap-[11px]">
        <InstanceIcon
          :instance="selected"
          class="h-[42px] w-[42px] rounded-[11px] text-[18px] shadow-[0_4px_14px_rgba(0,0,0,0.35)]"
        />
        <div class="min-w-0">
          <div class="truncate text-[14px] font-semibold text-[#eef1f5]">{{ selected.name }}</div>
          <div class="font-mono text-[11px] text-neutral-400">{{ subtitle }}</div>
        </div>
      </div>

      <button
        v-if="playing"
        type="button"
        :disabled="launching || stopping"
        class="flex w-full items-center justify-center gap-2 rounded-[11px] bg-[#e5484d] py-[11px] text-[14px] font-bold tracking-[0.02em] text-white transition hover:bg-[#ec5d62] active:scale-[0.98] disabled:opacity-60"
        @click="stop"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="1.5" /></svg>
        {{ stopping ? $t('common.loading') : $t('instanceCard.stop') }}
      </button>
      <template v-else>
        <div class="flex gap-2">
          <button
            type="button"
            :disabled="launching"
            class="flex flex-1 items-center justify-center gap-2 rounded-[11px] bg-[#3fb877] py-[11px] text-[14px] font-bold tracking-[0.02em] text-[#06210f] transition hover:bg-[#4bcb86] active:scale-[0.98] disabled:opacity-60"
            @click="play"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M7 5l13 7-13 7z" /></svg>
            {{ launching ? $t('common.loading') : $t('instanceCard.play') }}
          </button>
          <button
            v-if="resume"
            type="button"
            :disabled="launching"
            :title="$t('instanceCard.resume', { name: resumeLabel })"
            :aria-label="$t('instanceCard.resume', { name: resumeLabel })"
            class="flex w-[46px] shrink-0 items-center justify-center rounded-[11px] bg-[#3fb877]/15 text-[#3fb877] ring-1 ring-[#3fb877]/40 transition hover:bg-[#3fb877]/25 active:scale-[0.98] disabled:opacity-60"
            @click="playResume"
          >
            <UIcon name="i-lucide-log-in" class="size-4" />
          </button>
        </div>
        <p v-if="resume" class="mt-2 flex items-center gap-1.5 truncate text-[11px] text-neutral-400">
          <UIcon :name="resume.kind === 'Singleplayer' ? 'i-lucide-globe' : 'i-lucide-server'" class="size-3 shrink-0" />
          <span class="truncate">{{ resumeLabel }}</span>
        </p>
      </template>
    </template>

    <div v-else class="py-1 text-[12px] text-neutral-500">
      {{ $t('instanceCard.empty') }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { Instance } from '~/types/launcher'

const instances = useInstancesStore()
const activity = useActivityCenter()
const toast = useToast()

onMounted(() => {
  if (!instances.instances.length) instances.load()
})

const newestFirst = (list: Instance[]) =>
  [...list].sort((a, b) => (b.last_played || '').localeCompare(a.last_played || ''))

const running = computed(() => {
  const ids = new Set(activity.list.value.filter(a => a.kind === 'running').map(a => a.instanceId))
  return newestFirst(instances.instances.filter(i => ids.has(i.id)))[0]
})

const lastPlayed = computed(() => newestFirst(instances.instances.filter(i => i.last_played))[0])

const selected = computed<Instance | undefined>(() => running.value ?? lastPlayed.value)
const playing = computed(() => !!running.value)

const mc = useMinecraftLaunch(() => selected.value?.id)
const launching = computed(() => mc.launching.value)

const subtitle = computed(() => (selected.value ? instanceSubtitle(selected.value) : ''))

const play = async () => {
  if (!selected.value) return
  try {
    await mc.launch(selected.value.id)
  } catch {  }
}

const resume = computed(() => {
  const joined = selected.value?.last_joined
  return joined && selected.value && supportsQuickPlay(selected.value.mc_version) ? joined : null
})
const resumeLabel = computed(() => (resume.value ? lastJoinedLabel(resume.value) : ''))

const playResume = async () => {
  if (!selected.value || !resume.value) return
  try {
    await mc.launch(selected.value.id, quickPlayFor(resume.value))
  } catch {  }
}

const stopping = ref(false)

const stop = async () => {
  const id = selected.value?.id
  if (!id) return
  stopping.value = true
  try {
    await invoke('stop_instance', { id, force: false })
    if (!(await invoke<boolean>('is_instance_running', { id }))) activity.clear(id)
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    stopping.value = false
  }
}
</script>
