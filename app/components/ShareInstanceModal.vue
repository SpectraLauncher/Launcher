<template>
  <UModal v-model:open="isOpen" :title="$t('share.title', { name: target?.name ?? '' })" :ui="{ content: 'max-w-md' }">
    <template #body>
      <!-- before upload: what's about to be shared -->
      <div v-if="!result" class="space-y-4">
        <p class="text-sm text-muted">{{ $t('share.intro') }}</p>

        <div v-if="loadingPreview" class="py-8 text-center text-sm text-muted">{{ $t('common.loading') }}</div>

        <template v-else-if="preview">
          <div class="grid grid-cols-2 gap-2">
            <div class="rounded-lg border border-default p-3">
              <div class="text-xl font-bold">{{ preview.modrinth }}</div>
              <div class="text-xs text-muted">Modrinth</div>
            </div>
            <div class="rounded-lg border border-default p-3">
              <div class="text-xl font-bold">{{ preview.curseforge }}</div>
              <div class="text-xs text-muted">CurseForge</div>
            </div>
          </div>

          <div v-if="preview.unresolved.length" class="rounded-lg border border-default p-3">
            <USwitch
              v-model="includeUnresolved"
              :label="$t('share.includeUnresolved', { n: preview.unresolved.length })"
              :description="$t('share.includeUnresolvedDesc', { size: prettySize(preview.unresolved_bytes) })"
            />
            <ul class="mt-2 max-h-24 space-y-0.5 overflow-y-auto font-mono text-[11px] text-muted">
              <li v-for="f in preview.unresolved" :key="f" class="truncate">{{ f }}</li>
            </ul>
          </div>

          <p class="text-xs text-muted">{{ $t('share.privacyNote') }}</p>
        </template>

        <p v-if="error" class="text-sm text-error">{{ error }}</p>
      </div>

      <!-- after upload: the code -->
      <div v-else class="space-y-4 text-center">
        <div class="rounded-xl border border-default bg-elevated/40 px-4 py-4">
          <p class="text-[11px] tracking-wider text-muted uppercase">{{ $t('share.codeLabel') }}</p>
          <p class="mt-1 font-mono text-3xl font-bold tracking-[0.35em]">{{ result.code }}</p>
        </div>

        <div class="flex gap-2">
          <UButton
            class="flex-1"
            color="neutral"
            variant="soft"
            :icon="copied === 'code' ? 'i-lucide-check' : 'i-lucide-copy'"
            :label="$t('share.copyCode')"
            @click="copy('code', result.code)"
          />
          <UButton
            class="flex-1"
            color="neutral"
            variant="soft"
            :icon="copied === 'link' ? 'i-lucide-check' : 'i-lucide-link'"
            :label="$t('share.copyLink')"
            @click="copy('link', result.url)"
          />
        </div>

        <p class="font-mono text-[11px] break-all text-muted">{{ result.url }}</p>
        <p class="text-xs text-muted">{{ $t('share.expires', { days: daysLeft }) }}</p>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton variant="ghost" color="neutral" :label="result ? $t('common.close') : $t('common.cancel')" @click="close" />
        <UButton
          v-if="!result"
          icon="i-lucide-share-2"
          :label="$t('share.createBtn')"
          :loading="sharing"
          :disabled="loadingPreview"
          @click="doShare"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { SharePreview, ShareResult } from '~/types/launcher'

const { isOpen, target, close } = useShareModal()

const preview = ref<SharePreview | null>(null)
const result = ref<ShareResult | null>(null)
const includeUnresolved = ref(false)
const loadingPreview = ref(false)
const sharing = ref(false)
const error = ref<string | null>(null)
const copied = ref<'code' | 'link' | null>(null)

const daysLeft = computed(() =>
  result.value ? Math.max(1, Math.ceil((result.value.expires - Date.now()) / 86_400_000)) : 0,
)

function prettySize(bytes: number) {
  const mb = bytes / 1048576
  return mb < 1 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${mb.toFixed(1)} MB`
}

async function copy(what: 'code' | 'link', value: string) {
  await navigator.clipboard.writeText(value)
  copied.value = what
  setTimeout(() => (copied.value = null), 1500)
}

async function doShare() {
  if (!target.value) return
  sharing.value = true
  error.value = null
  try {
    result.value = await invoke<ShareResult>('share_instance', {
      id: target.value.id,
      includeUnresolved: includeUnresolved.value,
    })
  } catch (e) {
    error.value = String(e)
  } finally {
    sharing.value = false
  }
}

watch(isOpen, async (open) => {
  if (!open) return
  preview.value = null
  result.value = null
  error.value = null
  includeUnresolved.value = false
  if (!target.value) return

  loadingPreview.value = true
  try {
    preview.value = await invoke<SharePreview>('share_preview', { id: target.value.id })
  } catch (e) {
    error.value = String(e)
  } finally {
    loadingPreview.value = false
  }
})
</script>
