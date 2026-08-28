<template>
  <div class="mb-6 rounded-[15px] border border-primary-500/25 bg-linear-[160deg] from-primary-500/15 to-primary-500/5 p-[13px]">
    <div class="mb-[9px] text-[10px] font-semibold tracking-[0.12em] text-primary-300">
      {{ $t('instanceCard.title') }}
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
        type="button"
        :disabled="launching"
        class="flex w-full items-center justify-center gap-2 rounded-[11px] bg-[#3fb877] py-[11px] text-[14px] font-bold tracking-[0.02em] text-[#06210f] transition hover:bg-[#4bcb86] active:scale-[0.98] disabled:opacity-60"
        @click="play"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M7 5l13 7-13 7z" /></svg>
        {{ launching ? $t('common.loading') : $t('instanceCard.play') }}
      </button>
    </template>

    <div v-else class="py-1 text-[12px] text-neutral-500">
      {{ $t('instanceCard.empty') }}
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Instance } from '~/types/launcher'

const instances = useInstancesStore()

onMounted(() => {
  if (!instances.instances.length) instances.load()
})

const selected = computed<Instance | undefined>(() => {
  const played = instances.instances.filter(i => i.last_played)
  if (!played.length) return undefined
  return [...played].sort((a, b) => (b.last_played || '').localeCompare(a.last_played || ''))[0]
})

const mc = useMinecraftLaunch(() => selected.value?.id)
const launching = computed(() => mc.launching.value)

const subtitle = computed(() => (selected.value ? instanceSubtitle(selected.value) : ''))

const play = async () => {
  if (!selected.value) return
  try {
    await mc.launch(selected.value.id)
  } catch {  }
}
</script>
