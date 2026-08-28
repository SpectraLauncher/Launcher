<template>
  <div class="grid grid-cols-1 gap-4 md:grid-cols-[240px_1fr]">
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <p class="text-xs text-muted">{{ $t('content.count', { n: files.length }) }}</p>
        <UButton icon="i-lucide-refresh-cw" color="neutral" variant="ghost" size="xs" :loading="loading" square :title="$t('content.refresh')" @click="load" />
      </div>
      <div v-if="!files.length && !loading" class="py-8 text-center text-sm text-muted">{{ $t('logs.none') }}</div>
      <div v-else class="space-y-1">
        <button
          v-for="f in files"
          :key="f.rel"
          type="button"
          class="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-sm transition"
          :class="selected === f.rel ? 'bg-primary-500/15 text-primary-400' : 'text-neutral-300 hover:bg-white/5'"
          @click="open(f.rel)"
        >
          <UIcon :name="kindIcon(f.kind)" class="size-4 shrink-0" :class="f.kind === 'crash' ? 'text-error' : 'text-neutral-500'" />
          <span class="min-w-0 flex-1 truncate">{{ f.name }}</span>
          <UBadge v-if="f.kind === 'latest'" color="primary" variant="subtle" size="xs" :label="$t('logs.latest')" />
          <UBadge v-else-if="f.kind === 'crash'" color="error" variant="subtle" size="xs" :label="$t('logs.crash')" />
        </button>
      </div>
    </div>

    <div class="min-w-0">
      <div v-if="contentLoading" class="py-10 text-center text-sm text-muted">{{ $t('common.loading') }}</div>
      <div v-else-if="!selected" class="py-10 text-center text-sm text-muted">{{ $t('logs.selectHint') }}</div>
      <div v-else class="rounded-xl border border-default bg-black/30">
        <div class="flex items-center justify-between gap-2 border-b border-default px-3 py-1.5">
          <span class="truncate font-mono text-[11px] text-neutral-400">{{ selected }}</span>
          <div class="flex shrink-0 items-center gap-1">
            <UButton
              icon="i-lucide-share-2"
              color="primary"
              variant="ghost"
              size="xs"
              :loading="uploading"
              :label="$t('logs.share')"
              :title="$t('logs.shareHint')"
              @click="share"
            />
            <UButton icon="i-lucide-folder-open" color="neutral" variant="ghost" size="xs" :title="$t('content.openLocation')" square @click="reveal" />
          </div>
        </div>
        <pre class="max-h-[62vh] overflow-auto px-3 py-2 font-mono text-[11px] leading-relaxed whitespace-pre text-neutral-300">{{ content }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { openExternal as openUrl } from '~/utils/openExternal'
import type { LogFile } from '~/types/launcher'

const props = defineProps<{ instanceId: string; initialRel?: string | null }>()
const toast = useToast()
const { t } = useI18n()
const uploading = ref(false)

const files = ref<LogFile[]>([])
const loading = ref(false)
const selected = ref<string | null>(null)
const content = ref('')
const contentLoading = ref(false)

const kindIcon = (kind: LogFile['kind']) =>
  kind === 'crash' ? 'i-lucide-triangle-alert' : kind === 'archived' ? 'i-lucide-file-archive' : 'i-lucide-scroll-text'

async function load() {
  loading.value = true
  try {
    files.value = await invoke<LogFile[]>('list_log_files', { id: props.instanceId })
    const wantRel = props.initialRel || null
    const target = wantRel && files.value.some(f => f.rel === wantRel)
      ? wantRel
      : files.value[0]?.rel ?? null
    if (target && target !== selected.value) open(target)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    loading.value = false
  }
}

async function open(rel: string) {
  selected.value = rel
  contentLoading.value = true
  content.value = ''
  try {
    content.value = await invoke<string>('read_log_file', { id: props.instanceId, rel })
  } catch (e) {
    content.value = String(e)
  } finally {
    contentLoading.value = false
  }
}

async function reveal() {
  if (!selected.value) return
  try {
    const base = await invoke<string>('get_instance_path', { id: props.instanceId })
    await invoke('reveal_in_explorer', { path: `${base}/minecraft/${selected.value}` })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function share() {
  if (!selected.value) return
  uploading.value = true
  try {
    const paste = await invoke<{ id: string; url: string; raw: string }>('upload_log_to_mclogs', {
      id: props.instanceId,
      rel: selected.value,
    })
    try { await navigator.clipboard.writeText(paste.url) } catch {  }
    toast.add({
      title: t('logs.shared'),
      description: paste.url,
      color: 'success',
      actions: [{ label: t('logs.openLink'), onClick: () => openUrl(paste.url) }],
    })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    uploading.value = false
  }
}

watch(() => props.instanceId, load, { immediate: true })
</script>
