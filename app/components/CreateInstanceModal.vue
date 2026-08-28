<template>
  <UModal v-model:open="isOpen" :title="$t('create.title')" :ui="{ content: 'max-w-xl' }">
    <template #body>
      <div v-if="step === 'choice'" class="space-y-3">
        <p class="text-sm text-muted">{{ $t('create.chooseType') }}</p>
        <button
          v-for="c in choices"
          :key="c.key"
          type="button"
          class="flex w-full items-start gap-3 rounded-xl border border-default p-4 text-left transition hover:border-primary-500 hover:bg-primary-500/5"
          @click="selectChoice(c.key)"
        >
          <UIcon :name="c.icon" class="mt-0.5 size-5 shrink-0 text-primary-400" />
          <div>
            <div class="font-medium">{{ $t(c.title) }}</div>
            <div class="mt-0.5 text-sm text-muted">{{ $t(c.desc) }}</div>
          </div>
        </button>
      </div>

      <form v-else-if="step === 'custom'" class="space-y-4" @submit.prevent="submit">
        <div class="flex items-center gap-4">
          <div class="flex size-16 flex-none items-center justify-center overflow-hidden rounded-xl bg-white/5 ring-1 ring-white/10">
            <img v-if="form.iconPreview" :src="form.iconPreview" alt="" class="size-full object-cover" />
            <UIcon v-else name="i-lucide-box" class="size-7 text-neutral-400" />
          </div>
          <div class="flex flex-col gap-2">
            <UButton variant="soft" icon="i-lucide-palette" :label="$t('iconEditor.create')" @click="iconEditorOpen = true" />
            <UButton variant="soft" icon="i-lucide-upload" :label="$t('create.custom.chooseIcon')" @click="chooseIcon" />
            <UButton
              variant="ghost"
              color="neutral"
              icon="i-lucide-x"
              :label="$t('create.custom.removeIcon')"
              :disabled="!form.iconPreview"
              @click="clearIcon"
            />
          </div>

          <IconEditorModal v-model:open="iconEditorOpen" @saved="useDrawnIcon" />
        </div>

        <UFormField :label="$t('create.custom.name')">
          <UInput v-model="form.name" :placeholder="namePlaceholder" class="w-full" />
        </UFormField>

        <UFormField :label="$t('create.custom.loader')">
          <div class="flex flex-wrap gap-2">
            <button
              v-for="l in loaderItems"
              :key="l.value"
              type="button"
              class="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium transition"
              :class="form.loader === l.value
                ? 'bg-primary-500/15 text-primary-400 ring-1 ring-primary-500/40'
                : 'bg-white/5 text-neutral-300 hover:bg-white/10'"
              @click="form.loader = l.value"
            >
              <UIcon v-if="form.loader === l.value" name="i-lucide-check" class="size-3.5" />
              {{ l.label }}
            </button>
          </div>
        </UFormField>

        <UFormField :label="$t('create.custom.gameVersion')">
          <USelectMenu
            v-model="form.mcVersion"
            :items="mcVersionItems"
            :loading="loadingMc"
            :placeholder="$t('create.custom.gameVersionPlaceholder')"
            class="w-full"
          />
        </UFormField>

        <USwitch v-model="includeSnapshots" :label="$t('create.custom.snapshots')" />

        <div v-if="form.loader !== 'vanilla'" class="space-y-3 rounded-lg border border-default p-3">
          <UFormField :label="$t('create.custom.loaderVersion')">
            <URadioGroup v-model="form.loaderMode" :items="loaderModeItems" orientation="horizontal" />
          </UFormField>
          <USelectMenu
            v-if="form.loaderMode === 'other'"
            v-model="form.loaderExplicit"
            :items="loaderVersionItems"
            :loading="loadingLoader"
            :placeholder="$t('create.custom.pickLoaderVersion')"
            class="w-full"
          />
        </div>

        <p v-if="error" class="text-sm text-error">{{ error }}</p>
      </form>

      <div v-else-if="step === 'import'" class="space-y-5">
        <div>
          <p class="text-sm font-medium">{{ $t('create.import.fromCode') }}</p>
          <p class="mb-2 text-xs text-muted">{{ $t('create.import.fromCodeDesc') }}</p>
          <div class="flex gap-2">
            <UInput
              v-model="shareCode"
              placeholder="ABC123"
              maxlength="40"
              class="flex-1 font-mono tracking-widest uppercase"
              @keydown.enter="redeemCode"
            />
            <UButton
              icon="i-lucide-share-2"
              :label="$t('create.import.useCode')"
              :loading="redeeming"
              :disabled="shareCode.trim().length < 6"
              @click="redeemCode"
            />
          </div>
        </div>

        <div>
          <p class="text-sm font-medium">{{ $t('create.import.fromFile') }}</p>
          <p class="mb-2 text-xs text-muted">{{ $t('create.import.fromFileDesc') }}</p>
          <UButton
            variant="soft"
            icon="i-lucide-file-archive"
            :label="$t('create.import.chooseFile')"
            :loading="importingFile"
            @click="importFile"
          />
        </div>

        <div>
          <div class="mb-2 flex items-center justify-between gap-2">
            <div>
              <p class="text-sm font-medium">{{ $t('create.import.fromLauncher') }}</p>
              <p class="text-xs text-muted">{{ $t('create.import.fromLauncherDesc') }}</p>
            </div>
            <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-refresh-cw" :loading="scanning" square @click="scanExternal" />
          </div>

          <div v-if="scanning" class="py-6 text-center text-sm text-muted">{{ $t('common.loading') }}</div>
          <div v-else-if="!external.length" class="rounded-lg border border-dashed border-default py-6 text-center text-sm text-muted">
            {{ $t('create.import.none') }}
          </div>
          <div v-else class="max-h-64 space-y-1.5 overflow-y-auto">
            <div
              v-for="ext in external"
              :key="ext.path"
              class="flex items-center gap-3 rounded-lg border border-default p-2.5"
            >
              <UBadge :color="launcherColor(ext.launcher)" variant="subtle" size="xs" :label="launcherLabel(ext.launcher)" />
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm font-medium">{{ ext.name }}</div>
                <div class="truncate font-mono text-[11px] text-muted">
                  {{ ext.mc_version ?? '?' }}<span v-if="ext.loader"> · {{ ext.loader }}</span>
                </div>
              </div>
              <UButton
                size="xs"
                :label="$t('create.import.importBtn')"
                :loading="importingPath === ext.path"
                :disabled="!ext.mc_version || (!!importingPath && importingPath !== ext.path)"
                @click="importExternal(ext)"
              />
            </div>
          </div>
        </div>

        <p v-if="error" class="text-sm text-error">{{ error }}</p>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full items-center justify-between">
        <UButton
          v-if="step !== 'choice'"
          variant="ghost"
          color="neutral"
          icon="i-lucide-arrow-left"
          :label="$t('create.back')"
          @click="step = 'choice'"
        />
        <span v-else />

        <UButton
          v-if="step === 'custom'"
          icon="i-lucide-plus"
          :label="$t('create.create')"
          :loading="submitting"
          :disabled="!canSubmit"
          @click="submit"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import type { Loader, LoaderType, ExternalInstance, Instance, ShareImportResult } from '~/types/launcher'
import type { LoaderVersionMode } from '~/composables/useMinecraftMeta'

const { isOpen, pendingCode, close } = useCreateInstanceModal()
const toast = useToast()
const instances = useInstancesStore()
const meta = useMinecraftMeta()
const browser = useContentWindow()
const curseforge = useCurseforge()
const blockedModal = useBlockedModsModal()
const activity = useActivityCenter()
const router = useRouter()
const { t } = useI18n()

const external = ref<ExternalInstance[]>([])
const scanning = ref(false)
const importingFile = ref(false)
const importingPath = ref<string | null>(null)

const launcherLabel = (l: ExternalInstance['launcher']) =>
  ({ prism: 'Prism', curseforge: 'CurseForge', modrinth: 'Modrinth' }[l] ?? l)
const launcherColor = (l: ExternalInstance['launcher']) =>
  ({ prism: 'info', curseforge: 'warning', modrinth: 'success' }[l] ?? 'neutral') as 'info' | 'warning' | 'success' | 'neutral'

async function scanExternal() {
  scanning.value = true
  error.value = null
  try {
    external.value = await invoke<ExternalInstance[]>('detect_external_instances')
  } catch (e) {
    error.value = String(e)
  } finally {
    scanning.value = false
  }
}

async function importFile() {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Modpack', extensions: ['mrpack', 'zip'] }],
    })
    if (typeof selected !== 'string') return
    importingFile.value = true
    error.value = null
    const tid = activity.startTask(t('activity.importingModpack'))
    try {
      const instance = await invoke<Instance>('import_file', { path: selected, nameOverride: null })
      await instances.load()
      close()
      router.push(`/instance/${instance.id}`)
      const blocked = await curseforge.getBlocked(instance.id)
      if (blocked.length) blockedModal.open(instance.id)
    } finally {
      activity.endTask(tid)
      importingFile.value = false
    }
  } catch (e) {
    error.value = String(e)
  }
}

const shareCode = ref('')
const redeeming = ref(false)

async function redeemCode() {
  const code = shareCode.value.trim()
  if (code.length < 6 || redeeming.value) return
  redeeming.value = true
  error.value = null
  const tid = activity.startTask(t('activity.importingModpack'))
  try {
    const res = await invoke<ShareImportResult>('import_share', { code })
    await instances.load()
    close()
    router.push(`/instance/${res.instance.id}`)

    if (res.needs_curseforge) {
      toast.add({ title: t('share.needsCurseforge', { n: res.needs_curseforge }), color: 'warning' })
    }
    if (res.failed.length) {
      toast.add({ title: t('share.someFailed', { n: res.failed.length }), description: res.failed.join(', '), color: 'warning' })
    }
    const blocked = await curseforge.getBlocked(res.instance.id)
    if (blocked.length) blockedModal.open(res.instance.id)
  } catch (e) {
    error.value = String(e)
  } finally {
    activity.endTask(tid)
    redeeming.value = false
  }
}

async function importExternal(ext: ExternalInstance) {
  if (!ext.mc_version) return
  importingPath.value = ext.path
  error.value = null
  const tid = activity.startTask(t('activity.importingInstance', { name: ext.name }))
  try {
    const instance = await invoke<Instance>('import_external_instance', {
      name: ext.name,
      gameDir: ext.game_dir,
      mcVersion: ext.mc_version,
      loader: ext.loader,
      loaderVersion: ext.loader_version,
    })
    await instances.load()
    close()
    router.push(`/instance/${instance.id}`)
  } catch (e) {
    error.value = String(e)
  } finally {
    activity.endTask(tid)
    importingPath.value = null
  }
}

function browseModpacks() {
  browser.open({ kind: 'modpack', mode: 'createModpack' })
  close()
}

function selectChoice(key: Step) {
  if (key === 'modpack') browseModpacks()
  else step.value = key
}

type Step = 'choice' | 'custom' | 'modpack' | 'import'
const step = ref<Step>('choice')

const choices = [
  { key: 'custom' as Step, icon: 'i-lucide-wrench', title: 'create.choice.customTitle', desc: 'create.choice.customDesc' },
  { key: 'modpack' as Step, icon: 'i-lucide-package', title: 'create.choice.modpackTitle', desc: 'create.choice.modpackDesc' },
  { key: 'import' as Step, icon: 'i-lucide-download', title: 'create.choice.importTitle', desc: 'create.choice.importDesc' },
]

const form = reactive({
  name: '',
  iconPath: null as string | null,
  iconData: null as string | null,
  iconPreview: null as string | null,
  loader: 'vanilla' as LoaderType,
  mcVersion: '',
  loaderMode: 'stable' as LoaderVersionMode,
  loaderExplicit: '',
})
const includeSnapshots = ref(false)
const error = ref<string | null>(null)
const submitting = ref(false)

const loaderItems: { label: string; value: LoaderType }[] = [
  { label: 'Vanilla', value: 'vanilla' },
  { label: 'Fabric', value: 'fabric' },
  { label: 'NeoForge', value: 'neoforge' },
  { label: 'Forge', value: 'forge' },
  { label: 'Quilt', value: 'quilt' },
]
const loaderModeItems = computed(() => [
  { label: t('create.custom.stable'), value: 'stable' },
  { label: t('create.custom.latest'), value: 'latest' },
  { label: t('create.custom.other'), value: 'other' },
])

const mcVersions = ref<string[]>([])
const loadingMc = ref(false)
const mcVersionItems = computed(() => mcVersions.value)

const loaderVersions = ref<string[]>([])
const loadingLoader = ref(false)
const loaderVersionItems = computed(() => loaderVersions.value)

const namePlaceholder = computed(() => `${loaderLabel(form.loader)} ${form.mcVersion}`.trim())

const canSubmit = computed(() => {
  if (!form.mcVersion) return false
  if (form.loader !== 'vanilla' && form.loaderMode === 'other' && !form.loaderExplicit) return false
  return true
})

const iconEditorOpen = ref(false)

function useDrawnIcon(dataUrl: string) {
  form.iconData = dataUrl
  form.iconPreview = dataUrl
  form.iconPath = null
}

async function chooseIcon() {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg'] }],
    })
    if (typeof selected !== 'string') return
    form.iconPath = selected
    form.iconPreview = await invoke<string>('read_image_data_url', { path: selected })
  } catch (e) {
    error.value = String(e)
  }
}

function clearIcon() {
  form.iconPath = null
  form.iconData = null
  form.iconPreview = null
}

async function loadMcVersions() {
  loadingMc.value = true
  try {
    const list = await meta.getMinecraftVersions(includeSnapshots.value)
    mcVersions.value = list.map(v => v.id)
    if (!mcVersions.value.includes(form.mcVersion)) form.mcVersion = mcVersions.value[0] ?? ''
  } catch (e) {
    error.value = String(e)
  } finally {
    loadingMc.value = false
  }
}

async function loadLoaderVersions() {
  if (form.loader === 'vanilla' || !form.mcVersion) {
    loaderVersions.value = []
    return
  }
  loadingLoader.value = true
  try {
    const list = await meta.getLoaderVersions(form.loader, form.mcVersion)
    loaderVersions.value = list.map(v => v.version)
  } catch (e) {
    loaderVersions.value = []
    error.value = String(e)
  } finally {
    loadingLoader.value = false
  }
}

watch(
  () => [step.value, includeSnapshots.value] as const,
  ([s]) => {
    if (s === 'custom') loadMcVersions()
    else if (s === 'import' && !external.value.length) scanExternal()
  },
)

watch(
  () => [form.loader, form.mcVersion, form.loaderMode] as const,
  () => {
    form.loaderExplicit = ''
    if (form.loaderMode === 'other') loadLoaderVersions()
  },
)

watch(isOpen, (open) => {
  if (open) {
    if (pendingCode.value) {
      shareCode.value = pendingCode.value
      pendingCode.value = null
      step.value = 'import'
    }
    return
  }

  step.value = 'choice'
  shareCode.value = ''
  redeeming.value = false
  form.name = ''
  form.iconPath = null
  form.iconData = null
  form.iconPreview = null
  form.loader = 'vanilla'
  form.mcVersion = ''
  form.loaderMode = 'stable'
  form.loaderExplicit = ''
  includeSnapshots.value = false
  error.value = null
  external.value = []
  importingFile.value = false
  importingPath.value = null
})

async function submit() {
  if (!canSubmit.value) return
  error.value = null
  submitting.value = true
  try {
    let loader: Loader
    if (form.loader === 'vanilla') {
      loader = { type: 'vanilla' }
    } else {
      const version = await meta.resolveLoaderVersion(
        form.loader,
        form.mcVersion,
        form.loaderMode,
        form.loaderExplicit,
      )
      loader = { type: form.loader, version } as Loader
    }
    const created = await instances.create({
      name: form.name.trim() || namePlaceholder.value,
      mcVersion: form.mcVersion,
      loader,
      iconSourcePath: form.iconPath,
    })
    if (form.iconData) {
      await invoke('set_instance_icon_data', { id: created.id, dataUrl: form.iconData })
      created.icon = 'icon.png'
      invalidateInstanceIcon(created.id)
    }
    close()
  } catch (e) {
    error.value = String(e)
  } finally {
    submitting.value = false
  }
}
</script>
