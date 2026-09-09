<template>
  <UModal v-model:open="open" :dismissible="false" :ui="{ content: 'max-w-3xl' }">
    <template #content>
      <div class="grid grid-cols-1 sm:grid-cols-[1fr_360px]">
        <div class="flex flex-col gap-4 p-7">
          <span
            class="w-fit rounded-full bg-primary-500/15 px-3 py-1 text-xs font-semibold text-primary-400"
          >
            {{ $t('syncAnnounce.badge') }}
          </span>

          <h2 class="text-2xl font-bold tracking-tight">{{ $t('syncAnnounce.title') }}</h2>

          <p class="text-sm leading-relaxed text-muted">{{ $t('syncAnnounce.body') }}</p>
          <p class="text-sm leading-relaxed text-muted">{{ $t('syncAnnounce.later') }}</p>

          <div class="mt-auto flex flex-wrap gap-2 pt-2">
            <UButton
              icon="i-lucide-circle-slash"
              color="neutral"
              variant="soft"
              :label="$t('syncAnnounce.skip')"
              :disabled="busy"
              @click="skip"
            />
            <UButton
              icon="i-lucide-refresh-cw"
              :label="picked.length ? $t('syncAnnounce.syncSelected') : $t('syncAnnounce.syncAll')"
              :loading="busy"
              @click="begin"
            />
          </div>
        </div>

        <div class="flex flex-col gap-1 border-t border-default bg-white/2 p-5 sm:border-t-0 sm:border-l">
          <div class="mb-1 flex justify-end">
            <UButton
              icon="i-lucide-x"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              :title="$t('common.close')"
              :disabled="busy"
              @click="skip"
            />
          </div>

          <label
            v-for="option in SYNC_OPTIONS"
            :key="option"
            class="flex cursor-pointer items-center gap-3 rounded-lg px-2 py-2.5 transition hover:bg-white/4"
          >
            <span class="min-w-0 flex-1 text-sm font-semibold">
              {{ $t(`syncAnnounce.options.${option}`) }}
            </span>
            <USwitch :model-value="draft[option]" :disabled="busy" @update:model-value="draft[option] = $event" />
          </label>
        </div>
      </div>
    </template>
  </UModal>

  <SyncSourceModal v-model:open="sourceOpen" :sources="sources" @picked="apply" />
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { SYNC_OPTIONS, type SyncOption, type SyncSource } from '~/types/sync'

const sync = useInstanceSync()
const toast = useToast()
const { t } = useI18n()

const open = ref(false)
const busy = ref(false)
const sourceOpen = ref(false)
const sources = ref<SyncSource[]>([])

const draft = reactive(
  Object.fromEntries(SYNC_OPTIONS.map(o => [o, false])) as Record<SyncOption, boolean>,
)

const picked = computed(() => SYNC_OPTIONS.filter(o => draft[o]))

onMounted(async () => {
  let announce = false
  try {
    announce = await invoke<boolean>('take_sync_announcement')
  } catch {
    return
  }
  if (!announce) return

  try {
    sources.value = await sync.sources()
  } catch {
    return
  }
  if (sources.value.length < 2) return

  open.value = true
})

function skip() {
  open.value = false
}

function begin() {
  if (!sources.value.length) {
    open.value = false
    return
  }
  sourceOpen.value = true
}

async function apply(instanceId: string) {
  const chosen = picked.value.length ? picked.value : SYNC_OPTIONS
  busy.value = true
  const failed: SyncOption[] = []
  try {
    for (const option of chosen) {
      try {
        await sync.setGlobal(option, true, instanceId)
      } catch {
        failed.push(option)
      }
    }
  } finally {
    busy.value = false
  }

  open.value = false
  if (failed.length === chosen.length) {
    toast.add({ title: t('syncAnnounce.failed'), color: 'error' })
  } else {
    toast.add({ title: t('syncAnnounce.done', { n: chosen.length - failed.length }) })
  }
}
</script>
