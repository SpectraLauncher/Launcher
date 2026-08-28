<template>
  <UModal v-model:open="isOpen" :title="$t('changeLoader.title')" :ui="{ content: 'max-w-md' }">
    <template #body>
      <div v-if="instance" class="space-y-4">
        <p class="text-sm text-muted">{{ $t('changeLoader.desc', { mc: instance.mc_version }) }}</p>

        <div class="flex flex-wrap gap-2">
          <button
            v-for="l in loaderItems"
            :key="l.value"
            type="button"
            class="flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium transition"
            :class="loader === l.value
              ? 'bg-primary-500/15 text-primary-400 ring-1 ring-primary-500/40'
              : 'bg-white/5 text-neutral-300 hover:bg-white/10'"
            @click="loader = l.value"
          >
            <UIcon v-if="loader === l.value" name="i-lucide-check" class="size-3.5" />
            {{ l.label }}
          </button>
        </div>

        <div v-if="loader !== 'vanilla'" class="space-y-3 rounded-lg border border-default p-3">
          <URadioGroup v-model="loaderMode" :items="loaderModeItems" orientation="horizontal" />
          <USelectMenu
            v-if="loaderMode === 'other'"
            v-model="loaderExplicit"
            :items="loaderVersions"
            :loading="loadingVersions"
            :placeholder="$t('create.custom.pickLoaderVersion')"
            class="w-full"
          >
            <template #item-trailing="{ item }">
              <UBadge
                v-if="item === currentVersion"
                color="neutral"
                variant="subtle"
                size="xs"
                :label="$t('changeLoader.current')"
              />
            </template>
          </USelectMenu>
        </div>

        <p v-if="error" class="text-sm text-error">{{ error }}</p>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton variant="ghost" color="neutral" :label="$t('common.cancel')" @click="close" />
        <UButton :label="$t('changeLoader.apply')" :loading="saving" :disabled="!canApply" @click="apply" />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import type { Loader, LoaderType } from '~/types/launcher'
import type { LoaderVersionMode } from '~/composables/useMinecraftMeta'

const { isOpen, instanceId, close } = useChangeLoaderModal()
const instances = useInstancesStore()
const meta = useMinecraftMeta()
const toast = useToast()
const { t } = useI18n()

const instance = computed(() => instances.instances.find(i => i.id === instanceId.value) ?? null)

const loader = ref<LoaderType>('vanilla')
const loaderMode = ref<LoaderVersionMode>('stable')
const loaderExplicit = ref('')
const loaderVersions = ref<string[]>([])
const loadingVersions = ref(false)
const saving = ref(false)
const error = ref<string | null>(null)

const loaderItems: { label: string, value: LoaderType }[] = [
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

const currentVersion = computed(() => {
  const l = instance.value?.loader
  return l && l.type === loader.value && 'version' in l ? l.version : null
})

const canApply = computed(() => {
  if (loader.value !== 'vanilla' && loaderMode.value === 'other' && !loaderExplicit.value) return false
  return true
})

watch(isOpen, (open) => {
  if (open && instance.value) {
    loader.value = instance.value.loader.type
    loaderMode.value = 'stable'
    loaderExplicit.value = ''
    error.value = null
  }
})

watch([loader, loaderMode], () => {
  loaderExplicit.value = ''
  if (loader.value !== 'vanilla' && loaderMode.value === 'other') loadVersions()
})

async function loadVersions() {
  if (!instance.value || loader.value === 'vanilla') return
  loadingVersions.value = true
  try {
    const list = await meta.getLoaderVersions(loader.value, instance.value.mc_version)
    loaderVersions.value = list.map(v => v.version)
    if (currentVersion.value && loaderVersions.value.includes(currentVersion.value)) {
      loaderExplicit.value = currentVersion.value
    }
  } catch (e) {
    loaderVersions.value = []
    error.value = String(e)
  } finally {
    loadingVersions.value = false
  }
}

async function apply() {
  const inst = instance.value
  if (!inst) return
  saving.value = true
  error.value = null
  try {
    let newLoader: Loader
    if (loader.value === 'vanilla') {
      newLoader = { type: 'vanilla' }
    } else {
      const version = await meta.resolveLoaderVersion(loader.value, inst.mc_version, loaderMode.value, loaderExplicit.value)
      newLoader = { type: loader.value, version } as Loader
    }
    await instances.update({ ...inst, loader: newLoader })
    toast.add({ title: t('changeLoader.done'), color: 'success' })
    close()
  } catch (e) {
    error.value = String(e)
  } finally {
    saving.value = false
  }
}
</script>
