<template>
  <UModal v-model:open="open" :ui="{ content: 'max-w-4xl' }" :dismissible="false">
    <template #content>
      <div class="flex flex-col">
        <!-- Header -->
        <div class="flex items-start gap-4 border-b border-default px-5 py-4">
          <!-- Animated crash icon -->
          <div class="mt-0.5 flex size-10 shrink-0 items-center justify-center rounded-xl bg-red-500/15">
            <UIcon name="i-lucide-triangle-alert" class="size-5 text-red-400" />
          </div>
          <div class="min-w-0 flex-1">
            <h2 class="text-base font-semibold text-white">{{ $t('crash.title') }}</h2>
            <p class="mt-0.5 truncate text-sm text-neutral-400">
              {{ subtitle }}
            </p>
          </div>
          <!-- Exit code badge -->
          <UBadge
            v-if="crash?.code != null"
            color="error"
            variant="subtle"
            class="mt-0.5 shrink-0 font-mono"
            :label="`exit ${crash.code}`"
          />
        </div>

        <!-- Crash report content -->
        <div class="min-h-0 flex-1 px-5 py-4">
          <!-- File name -->
          <div v-if="crash?.crash_report_rel" class="mb-2 flex items-center gap-2">
            <UIcon name="i-lucide-file-text" class="size-3.5 shrink-0 text-red-400" />
            <span class="truncate font-mono text-[11px] text-neutral-500">{{ crash.crash_report_rel }}</span>
          </div>

          <!-- Loading -->
          <div v-if="contentLoading" class="flex items-center justify-center py-10">
            <UIcon name="i-lucide-loader-circle" class="size-6 animate-spin text-neutral-500" />
          </div>

          <!-- No report generated -->
          <div
            v-else-if="!crash?.crash_report_rel"
            class="flex flex-col items-center gap-2 py-10 text-center"
          >
            <UIcon name="i-lucide-file-x" class="size-8 text-neutral-600" />
            <p class="text-sm text-neutral-500">{{ $t('crash.noReport') }}</p>
          </div>

          <!-- Load error -->
          <div
            v-else-if="loadError"
            class="flex flex-col items-center gap-2 py-10 text-center"
          >
            <UIcon name="i-lucide-circle-x" class="size-8 text-red-500/60" />
            <p class="text-sm text-neutral-500">{{ $t('crash.loadError') }}</p>
          </div>

          <!-- Report text with coloring -->
          <div v-else class="overflow-hidden rounded-xl border border-default bg-black/40">
            <!-- Toolbar -->
            <div class="flex items-center justify-between gap-2 border-b border-default px-3 py-1.5">
              <span class="truncate font-mono text-[11px] text-neutral-500">
                {{ crash.crash_report_rel?.split('/').at(-1) }}
              </span>
              <UButton
                icon="i-lucide-copy"
                color="neutral"
                variant="ghost"
                size="xs"
                square
                :title="$t('common.copy')"
                @click="copyReport"
              />
            </div>
            <!-- Scrollable content -->
            <pre
              class="max-h-[42vh] overflow-auto px-3 py-2.5 font-mono text-[11px] leading-relaxed"
            ><span
  v-for="(line, i) in coloredLines"
  :key="i"
  :class="line.cls"
  class="block whitespace-pre"
>{{ line.text }}</span></pre>
          </div>
        </div>

        <!-- Footer actions -->
        <div class="flex items-center justify-between gap-2 border-t border-default px-5 py-3">
          <div class="flex items-center gap-2">
            <!-- Share on mclo.gs -->
            <UButton
              v-if="crash?.crash_report_rel"
              icon="i-lucide-share-2"
              color="primary"
              variant="soft"
              size="sm"
              :loading="uploading"
              :label="$t('crash.share')"
              @click="share"
            />
            <!-- Open in Logs tab -->
            <UButton
              v-if="crash?.crash_report_rel"
              icon="i-lucide-scroll-text"
              color="neutral"
              variant="ghost"
              size="sm"
              :label="$t('crash.openLogs')"
              @click="openLogs"
            />
          </div>
          <UButton
            color="neutral"
            variant="solid"
            size="sm"
            :label="$t('crash.dismiss')"
            @click="dismiss"
          />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { openExternal as openUrl } from '~/utils/openExternal'

const ac = useActivityCenter()
const instances = useInstancesStore()
const router = useRouter()
const toast = useToast()
const { t } = useI18n()

const open = ac.crashOpen
const instanceId = ac.crashInstance

// The current crash info (driven by ac.crashFor, reactive on instanceId).
const crash = computed(() => (instanceId.value ? ac.crashFor(instanceId.value).value : null))

const instanceName = computed(
  () => instances.instances.find(i => i.id === instanceId.value)?.name ?? instanceId.value ?? '',
)

const subtitle = computed(() => {
  if (!crash.value) return ''
  const name = instanceName.value
  return crash.value.code != null
    ? t('crash.subtitle', { name, code: crash.value.code })
    : t('crash.subtitleNoCode', { name })
})

// --- Report content ---
const reportContent = ref('')
const contentLoading = ref(false)
const loadError = ref(false)

async function loadReport(rel: string | null) {
  reportContent.value = ''
  loadError.value = false
  if (!rel || !instanceId.value) return
  contentLoading.value = true
  try {
    reportContent.value = await invoke<string>('read_log_file', { id: instanceId.value, rel })
  } catch {
    loadError.value = true
  } finally {
    contentLoading.value = false
  }
}

watch(
  [open, () => crash.value?.crash_report_rel],
  ([isOpen, rel]) => {
    if (isOpen) loadReport(rel ?? null)
  },
  { immediate: true },
)

// --- Line coloring (mirrors LiveLogsModal logic + crash report specifics) ---
function lineClass(line: string): string {
  if (/^-- (Head|Affected levels|Suspected Mods|Stack Trace|System Details)/.test(line))
    return 'text-amber-300 font-semibold'
  if (/^Description:/.test(line)) return 'text-red-300 font-semibold'
  if (/^\s*at\s|Exception|Caused by:|Error/.test(line)) return 'text-red-400'
  if (/^Time:|^Description:|^A detailed walkthrough/.test(line)) return 'text-neutral-300'
  if (/^\s*\.\.\. \d+ more/.test(line)) return 'text-neutral-500'
  if (/^-{10,}/.test(line.trim())) return 'text-neutral-600'
  return 'text-neutral-400'
}

const coloredLines = computed(() =>
  reportContent.value.split('\n').map(text => ({ text, cls: lineClass(text) })),
)

// --- Actions ---
const uploading = ref(false)

async function share() {
  if (!crash.value?.crash_report_rel || !instanceId.value) return
  uploading.value = true
  try {
    const paste = await invoke<{ id: string; url: string; raw: string }>('upload_log_to_mclogs', {
      id: instanceId.value,
      rel: crash.value.crash_report_rel,
    })
    try { await navigator.clipboard.writeText(paste.url) } catch { /* clipboard optional */ }
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

async function openLogs() {
  if (!instanceId.value) return
  dismiss()
  await router.push(`/instance/${instanceId.value}?tab=logs&crashRel=${encodeURIComponent(crash.value?.crash_report_rel ?? '')}`)
}

async function copyReport() {
  try {
    await navigator.clipboard.writeText(reportContent.value)
    toast.add({ title: t('common.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

function dismiss() {
  if (instanceId.value) ac.clearCrash(instanceId.value)
  open.value = false
}
</script>
