<template>
  <div v-if="selected" class="flex flex-wrap items-center gap-4 rounded-xl border border-default bg-muted p-4">
    <InstanceIcon :instance="selected" class="size-16 rounded-xl text-[24px]" />
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        <span class="truncate text-base font-semibold text-highlighted">{{ selected.name }}</span>
        <UBadge v-if="playing" color="primary" variant="subtle" size="sm" :label="$t('instanceCard.nowPlaying')" />
      </div>
      <div class="mt-0.5 truncate text-[13px] tabular-nums text-muted">{{ subtitle }}</div>
      <p v-if="resume && !playing" class="mt-1.5 flex items-center gap-1.5 truncate text-[13px] text-toned">
        <UIcon :name="resume.kind === 'Singleplayer' ? 'i-lucide-globe' : 'i-lucide-server'" class="size-3.5 shrink-0 text-dimmed" />
        <span class="truncate">{{ resumeLabel }}</span>
      </p>
    </div>

    <div class="flex items-center gap-2">
      <UButton
        v-if="playing"
        color="error"
        size="lg"
        icon="i-lucide-square"
        :loading="stopping"
        :label="$t('instanceCard.stop')"
        class="px-5"
        @click="stop"
      />
      <template v-else>
        <UButton
          v-if="resume"
          color="neutral"
          variant="soft"
          size="lg"
          icon="i-lucide-log-in"
          :disabled="launching"
          :label="$t('instanceCard.resume', { name: resumeLabel })"
          class="max-w-72"
          :ui="{ label: 'truncate' }"
          @click="playResume"
        />
        <UButton
          size="xl"
          :loading="launching"
          :label="$t('instanceCard.play')"
          class="px-7"
          @click="play"
        >
          <template #leading>
            <svg v-if="!launching" class="size-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l13-7.5z" /></svg>
          </template>
        </UButton>
      </template>
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

const subtitle = computed(() => {
  if (!selected.value) return ''
  const hours = Math.round((selected.value.playtime_seconds ?? 0) / 360) / 10
  return hours ? `${instanceSubtitle(selected.value)} · ${hours} h` : instanceSubtitle(selected.value)
})

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
