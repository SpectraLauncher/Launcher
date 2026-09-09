<template>
  <UModal v-model:open="open" :title="$t('sync.sourceTitle')" :ui="{ content: 'max-w-lg' }">
    <template #body>
      <div class="space-y-3">
        <p class="text-sm text-muted">{{ $t('sync.sourceDesc') }}</p>

        <UInput
          v-model="filter"
          icon="i-lucide-search"
          variant="soft"
          :placeholder="$t('sync.sourceSearch')"
          class="w-full"
        />

        <div class="max-h-72 space-y-1 overflow-y-auto">
          <button
            v-for="s in filtered"
            :key="s.id"
            type="button"
            class="flex w-full items-center gap-2.5 rounded-lg border px-3 py-2 text-left transition"
            :class="picked === s.id
              ? 'border-primary-500/60 bg-primary-500/10'
              : 'border-transparent hover:bg-white/5'"
            @click="picked = s.id"
          >
            <UIcon name="i-lucide-box" class="size-4 shrink-0 text-neutral-500" />
            <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ s.name }}</span>
            <span class="shrink-0 font-mono text-[11px] text-neutral-500">{{ s.mc_version }}</span>
            <UIcon
              v-if="picked === s.id"
              name="i-lucide-circle-check"
              class="size-4 shrink-0 text-primary-400"
            />
          </button>
          <p v-if="!filtered.length" class="py-6 text-center text-sm text-muted">
            {{ $t('sync.sourceEmpty') }}
          </p>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" :label="$t('common.cancel')" @click="cancel" />
        <UButton
          icon="i-lucide-refresh-cw"
          :disabled="!picked"
          :label="$t('sync.sourceConfirm')"
          @click="confirm"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import type { SyncSource } from '~/types/sync'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ sources: SyncSource[] }>()
const emit = defineEmits<{ picked: [string] }>()

const filter = ref('')
const picked = ref<string | null>(null)

const filtered = computed(() => {
  const needle = filter.value.trim().toLowerCase()
  if (!needle) return props.sources
  return props.sources.filter(s => s.name.toLowerCase().includes(needle))
})

watch(open, (isOpen) => {
  if (isOpen) {
    filter.value = ''
    picked.value = props.sources[0]?.id ?? null
  }
})

function cancel() {
  open.value = false
}

function confirm() {
  if (!picked.value) return
  emit('picked', picked.value)
  open.value = false
}
</script>
