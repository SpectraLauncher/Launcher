<template>
  <UModal v-model:open="isOpen" :title="$t('blocked.title')" :ui="{ content: 'max-w-xl' }">
    <template #body>
      <div class="space-y-4">
        <p class="text-sm text-muted">{{ $t('blocked.desc') }}</p>

        <!-- all done -->
        <div v-if="!blocked.length" class="flex flex-col items-center gap-2 py-8 text-center">
          <UIcon name="i-lucide-circle-check" class="size-9 text-emerald-400" />
          <p class="text-sm font-medium">{{ $t('blocked.allDone') }}</p>
        </div>

        <template v-else>
          <!-- mod list -->
          <div class="max-h-60 space-y-1.5 overflow-y-auto">
            <div
              v-for="b in blocked"
              :key="b.file_id"
              class="flex items-center gap-2.5 rounded-lg border border-default p-2.5"
            >
              <UIcon name="i-lucide-file-warning" class="size-4 shrink-0 text-amber-400" />
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm font-medium">{{ b.name }}</div>
                <div class="truncate font-mono text-[11px] text-muted">{{ b.filename }}</div>
              </div>
              <UButton
                icon="i-lucide-external-link"
                color="primary"
                variant="soft"
                size="xs"
                :label="$t('blocked.download')"
                @click="openUrl(b.url)"
              />
            </div>
          </div>

          <!-- scan folder -->
          <div class="space-y-1.5 rounded-lg border border-dashed border-default p-3">
            <div class="flex items-center justify-between gap-2">
              <span class="flex items-center gap-1.5 text-xs text-muted">
                <UIcon v-if="scanning" name="i-lucide-loader-circle" class="size-3.5 animate-spin text-primary-400" />
                <UIcon v-else name="i-lucide-folder-search" class="size-3.5" />
                {{ $t('blocked.watching') }}
              </span>
              <div class="flex gap-1.5">
                <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-folder" :label="$t('blocked.chooseFolder')" @click="chooseFolder" />
                <UButton size="xs" color="neutral" variant="soft" icon="i-lucide-refresh-cw" :loading="scanning" :label="$t('blocked.rescan')" @click="rescan(false)" />
              </div>
            </div>
            <p class="truncate font-mono text-[11px] text-neutral-500">{{ scanDir || '—' }}</p>
          </div>
        </template>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full items-center justify-between gap-2">
        <span class="text-xs text-muted">{{ $t('blocked.progress', { done: resolvedCount, total: total }) }}</span>
        <UButton color="neutral" :label="blocked.length ? $t('blocked.later') : $t('common.close')" @click="close" />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { openExternal as openUrl } from '~/utils/openExternal'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import type { BlockedMod } from '~/types/launcher'

const { isOpen, instanceId, close, notifyResolved } = useBlockedModsModal()
const toast = useToast()
const { t } = useI18n()

const blocked = ref<BlockedMod[]>([])
const scanDir = ref<string | null>(null)
const scanning = ref(false)
const total = ref(0)
const resolvedCount = computed(() => total.value - blocked.value.length)

let timer: ReturnType<typeof setInterval> | null = null

watch(isOpen, async (open) => {
  if (open) {
    blocked.value = await invoke<BlockedMod[]>('get_blocked_mods', { instanceId: instanceId.value })
    total.value = blocked.value.length
    if (!scanDir.value) {
      scanDir.value = await invoke<string | null>('default_downloads_dir')
    }
    // Poll the watched folder while open (approximates a live watcher).
    timer = setInterval(() => { if (blocked.value.length) rescan(true) }, 3000)
  } else if (timer) {
    clearInterval(timer)
    timer = null
  }
})
onBeforeUnmount(() => { if (timer) clearInterval(timer) })

async function rescan(quiet: boolean) {
  if (!instanceId.value || scanning.value || !blocked.value.length) return
  scanning.value = true
  try {
    const res = await invoke<{ resolved: number, remaining: BlockedMod[] }>('resolve_blocked_mods', {
      instanceId: instanceId.value,
      dir: scanDir.value,
    })
    blocked.value = res.remaining
    if (res.resolved > 0) {
      toast.add({ title: t('blocked.resolved', { n: res.resolved }), color: 'success' })
      notifyResolved()
    }
  } catch (e) {
    if (!quiet) toast.add({ title: String(e), color: 'error' })
  } finally {
    scanning.value = false
  }
}

async function chooseFolder() {
  const picked = await openDialog({ directory: true, multiple: false })
  if (typeof picked === 'string') {
    scanDir.value = picked
    rescan(false)
  }
}
</script>
