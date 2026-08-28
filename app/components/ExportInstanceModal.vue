<template>
  <UModal v-model:open="isOpen" :title="$t('export.title', { name: target?.name ?? '' })" :ui="{ content: 'max-w-xl' }">
    <template #body>
      <div class="space-y-5">
        <div>
          <p class="mb-2 text-sm font-medium">{{ $t('export.format') }}</p>
          <div class="grid grid-cols-2 gap-2">
            <button
              v-for="f in formats"
              :key="f.value"
              type="button"
              class="rounded-lg border px-3 py-2.5 text-left transition"
              :class="format === f.value ? 'border-primary-500 bg-primary-500/10' : 'border-default hover:border-neutral-500'"
              @click="format = f.value"
            >
              <div class="flex items-center gap-1.5 text-sm font-medium">
                <UIcon :name="f.icon" class="size-4" />
                {{ f.label }}
              </div>
              <div class="mt-0.5 text-xs text-muted">{{ f.desc }}</div>
            </button>
          </div>
        </div>

        <template v-if="isModpack">
          <div class="grid grid-cols-2 gap-3">
            <UFormField :label="$t('export.version')">
              <UInput v-model="version" placeholder="1.0.0" class="w-full" />
            </UFormField>
            <UFormField :label="isCf ? $t('export.author') : $t('export.summary')">
              <UInput v-model="meta" :placeholder="$t('export.summaryPlaceholder')" class="w-full" />
            </UFormField>
          </div>
          <USwitch v-model="optionalDisabled" :label="$t('export.optionalDisabled')" :description="$t('export.optionalDisabledDesc')" />
        </template>

        <div>
          <p class="mb-2 text-sm font-medium">{{ $t('export.include') }}</p>
          <div class="max-h-64 overflow-y-auto rounded-lg border border-default p-1.5">
            <FileTree
              v-if="target"
              :key="target.id"
              :instance-id="target.id"
              :excluded="excluded"
              :included="included"
              :toggle="toggle"
            />
          </div>
          <p v-if="isModpack" class="mt-2 text-xs text-muted">{{ $t('export.mrpackHint') }}</p>
        </div>

        <p v-if="error" class="text-sm text-error">{{ error }}</p>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton variant="ghost" color="neutral" :label="$t('common.cancel')" @click="close" />
        <UButton
          icon="i-lucide-package"
          :label="$t('export.exportBtn')"
          :loading="exporting"
          @click="doExport"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'

const { isOpen, target, close } = useExportModal()
const curseforge = useCurseforge()
const activity = useActivityCenter()
const toast = useToast()
const { t } = useI18n()

type Format = 'mrpack' | 'curseforge' | 'backup'
const format = ref<Format>('mrpack')
const version = ref('1.0.0')
const meta = ref('')
const optionalDisabled = ref(false)
const excluded = ref<Set<string>>(new Set())
const included = ref<Set<string>>(new Set())
const exporting = ref(false)
const error = ref<string | null>(null)
const cfEnabled = ref(false)

const isCf = computed(() => format.value === 'curseforge')
const isModpack = computed(() => format.value !== 'backup')

const DEFAULT_EXCLUDE = ['saves', 'screenshots', 'crash-reports', 'backups']

const formats = computed<{ value: Format, label: string, desc: string, icon: string }[]>(() => {
  const list = [{ value: 'mrpack' as Format, label: 'Modrinth (.mrpack)', desc: t('export.mrpackDesc'), icon: 'i-lucide-package' }]
  if (cfEnabled.value) list.push({ value: 'curseforge' as Format, label: 'CurseForge (.zip)', desc: t('export.cfDesc'), icon: 'i-lucide-flame' })
  list.push({ value: 'backup' as Format, label: t('export.backup'), desc: t('export.backupDesc'), icon: 'i-lucide-archive' })
  return list
})

function isIncluded(path: string): boolean {
  const parts = path.split('/')
  for (let i = parts.length; i >= 1; i--) {
    const p = parts.slice(0, i).join('/')
    if (included.value.has(p)) return true
    if (excluded.value.has(p)) return false
  }
  return true
}

function toggle(path: string, _isDir: boolean) {
  const wasIncluded = isIncluded(path)
  const ex = new Set(excluded.value)
  const inc = new Set(included.value)
  for (const e of [...ex]) if (e === path || e.startsWith(path + '/')) ex.delete(e)
  for (const e of [...inc]) if (e === path || e.startsWith(path + '/')) inc.delete(e)
  if (wasIncluded) ex.add(path)
  else inc.add(path)
  excluded.value = ex
  included.value = inc
}

watch(isOpen, (open) => {
  if (open && target.value) {
    format.value = 'mrpack'
    version.value = '1.0.0'
    meta.value = ''
    optionalDisabled.value = false
    error.value = null
    excluded.value = new Set(DEFAULT_EXCLUDE)
    included.value = new Set()
    curseforge.enabled().then(v => (cfEnabled.value = v)).catch(() => (cfEnabled.value = false))
  }
})

async function doExport() {
  const tgt = target.value
  if (!tgt) return
  error.value = null

  const ext = format.value === 'mrpack' ? 'mrpack' : 'zip'
  const filterName = format.value === 'mrpack'
    ? 'Modrinth modpack'
    : format.value === 'curseforge' ? 'CurseForge modpack' : 'Spectra backup'
  const safe = tgt.name.replace(/[^\w.\- ]+/g, '_').trim() || 'instance'

  try {
    const dest = await save({
      defaultPath: `${safe}.${ext}`,
      filters: [{ name: filterName, extensions: [ext] }],
    })
    if (typeof dest !== 'string') return

    exporting.value = true
    const tid = activity.startTask(t('activity.exportingInstance', { name: tgt.name }))
    const exclude = [...excluded.value]
    const include = [...included.value]
    try {
      if (format.value === 'mrpack') {
        await invoke('export_mrpack', {
          id: tgt.id, dest, version: version.value || null, summary: meta.value || null, exclude, include, optionalDisabled: optionalDisabled.value,
        })
      } else if (format.value === 'curseforge') {
        await invoke('export_curseforge', {
          id: tgt.id, dest, version: version.value || null, author: meta.value || null, exclude, include, optionalDisabled: optionalDisabled.value,
        })
      } else {
        await invoke('export_instance', { id: tgt.id, dest, exclude, include })
      }
      toast.add({ title: t('export.done'), color: 'success' })
      close()
    } finally {
      activity.endTask(tid)
      exporting.value = false
    }
  } catch (e) {
    error.value = String(e)
  }
}
</script>
