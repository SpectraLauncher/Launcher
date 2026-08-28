<template>
  <div v-if="form" class="grid h-full min-h-0 grid-cols-1 gap-6 md:grid-cols-[200px_1fr]">
    <nav class="flex flex-col gap-1 overflow-y-auto py-4 pl-4">
      <button
        v-for="s in sections"
        :key="s.key"
        type="button"
        class="flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium transition"
        :class="section === s.key ? 'bg-primary-500/15 text-primary-400' : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
        @click="section = s.key"
      >
        <UIcon :name="s.icon" class="size-4" />
        {{ $t(s.label) }}
      </button>
    </nav>

    <div class="min-w-0 space-y-6 overflow-y-auto py-4 pr-4">
      <template v-if="section === 'general'">
        <div class="flex flex-wrap items-start gap-6">
          <UFormField
            class="min-w-[220px] flex-1"
            :label="$t('instSettings.name')"
            :description="$t('instSettings.nameDesc')"
          >
            <UInput v-model="form.name" class="w-full" />
          </UFormField>

          <UFormField :label="$t('instSettings.icon')">
            <UDropdownMenu :items="iconMenu">
              <button
                type="button"
                class="group/icon relative size-24 shrink-0 cursor-pointer overflow-hidden rounded-2xl border border-default"
                :title="$t('instance.changeIcon')"
                :disabled="pickingIcon"
              >
                <InstanceIcon v-if="instance" :key="iconKey" :instance="instance" class="size-full rounded-2xl text-3xl" />
                <span
                  class="absolute inset-0 flex items-center justify-center bg-black/55 opacity-0 transition group-hover/icon:opacity-100"
                >
                  <UIcon
                    :name="pickingIcon ? 'i-lucide-loader-circle' : 'i-lucide-pencil'"
                    class="size-5 text-white"
                    :class="pickingIcon && 'animate-spin'"
                  />
                </span>
              </button>
            </UDropdownMenu>
          </UFormField>

          <IconEditorModal v-model:open="iconEditorOpen" :instance-id="props.instanceId" @saved="onIconChanged" />
        </div>

        <div>
          <p class="mb-1.5 text-sm font-medium">{{ $t('changeLoader.menu') }}</p>
          <div class="flex items-center gap-2">
            <span class="rounded-md bg-white/7 px-2.5 py-1 text-sm font-medium">{{ loaderLabel(form.loader.type) }}<span v-if="'version' in form.loader" class="ml-1 font-mono text-xs opacity-70">{{ form.loader.version }}</span></span>
            <UButton icon="i-lucide-layers" color="neutral" variant="soft" size="sm" :label="$t('changeLoader.change')" @click="changeLoaderModal.open(props.instanceId)" />
          </div>
          <p class="mt-1.5 text-xs text-muted">{{ $t('changeLoader.settingsHint') }}</p>
        </div>

        <div>
          <p class="mb-1.5 text-sm font-medium">{{ $t('instSettings.duplicate') }}</p>
          <UButton icon="i-lucide-copy" color="neutral" variant="soft" :loading="busy" :label="$t('instSettings.duplicate')" @click="duplicate" />
          <p class="mt-1.5 text-xs text-muted">{{ $t('instSettings.duplicateHint') }}</p>
        </div>

        <div>
          <p class="mb-1.5 text-sm font-medium text-error">{{ $t('instSettings.delete') }}</p>
          <UButton icon="i-lucide-trash-2" color="error" :label="$t('instSettings.delete')" @click="remove" />
          <p class="mt-1.5 text-xs text-muted">{{ $t('instSettings.deleteHint') }}</p>
        </div>
      </template>

      <template v-else-if="section === 'install'">
        <div class="rounded-xl border border-default bg-white/3 p-4 text-sm">
          <div class="flex justify-between py-1"><span class="text-muted">{{ $t('instSettings.platform') }}</span><span class="font-medium">{{ loaderLabel(form.loader.type) }}</span></div>
          <div class="flex justify-between py-1"><span class="text-muted">{{ $t('instSettings.gameVersion') }}</span><span class="font-mono">{{ form.mc_version }}</span></div>
          <div v-if="loaderVersion" class="flex justify-between py-1"><span class="text-muted">{{ $t('instSettings.loaderVersion') }}</span><span class="font-mono">{{ loaderVersion }}</span></div>
        </div>
        <div>
          <p class="mb-1.5 text-sm font-medium">{{ $t('instSettings.repair') }}</p>
          <UButton icon="i-lucide-wrench" color="neutral" variant="soft" :loading="repairing" :label="$t('instSettings.repair')" @click="repair" />
          <p class="mt-1.5 text-xs text-muted">{{ $t('instSettings.repairHint') }}</p>
        </div>
      </template>

      <template v-else-if="section === 'window'">
        <USwitch v-model="form.override_window" :label="$t('instSettings.customWindow')" :description="$t('instSettings.customWindowDesc')" />
        <fieldset :disabled="!form.override_window" class="space-y-4" :class="{ 'opacity-50': !form.override_window }">
          <div class="flex items-center justify-between gap-4">
            <div>
              <p class="text-sm font-medium">{{ $t('instSettings.fullscreen') }}</p>
              <p class="text-xs text-muted">{{ $t('instSettings.fullscreenDesc') }}</p>
            </div>
            <USwitch v-model="form.fullscreen" />
          </div>
          <UFormField :label="$t('instSettings.width')" :description="$t('instSettings.widthDesc')">
            <UInput v-model.number="form.width" type="number" placeholder="854" class="w-40" />
          </UFormField>
          <UFormField :label="$t('instSettings.height')" :description="$t('instSettings.heightDesc')">
            <UInput v-model.number="form.height" type="number" placeholder="480" class="w-40" />
          </UFormField>
        </fieldset>
      </template>

      <template v-else-if="section === 'java'">
        <div class="flex items-start gap-3 rounded-xl border border-default bg-white/3 p-3">
          <UIcon name="i-lucide-coffee" class="mt-0.5 size-5 shrink-0 text-primary-400" />
          <div class="min-w-0 text-sm">
            <p class="font-medium">{{ $t('instSettings.requiredJava', { version: form.mc_version, major: requiredJava }) }}</p>
            <p v-if="matchedJava" class="mt-0.5 text-xs text-success">
              {{ $t('instSettings.javaDetected', { label: `Java ${matchedJava.major} (${matchedJava.version})` }) }}
            </p>
            <p v-else class="mt-0.5 text-xs text-muted">{{ $t('instSettings.javaWillDownload') }}</p>
          </div>
        </div>

        <div class="space-y-2">
          <USwitch v-model="form.override_java" :label="$t('instSettings.customJava')" :description="$t('instSettings.customJavaDesc')" />
          <fieldset :disabled="!form.override_java" :class="{ 'opacity-50': !form.override_java }">
            <div class="flex gap-2">
              <UInput v-model="form.java_path" placeholder="C:\\...\\javaw.exe" class="flex-1" />
              <UButton icon="i-lucide-folder" color="neutral" variant="soft" @click="browseJava" />
            </div>
            <p class="mt-1 text-xs text-muted">{{ $t('instSettings.javaPathNote') }}</p>
          </fieldset>
        </div>

        <div class="space-y-2">
          <USwitch v-model="form.override_memory" :label="$t('instSettings.customMemory')" :description="$t('instSettings.customMemoryDesc')" />
          <fieldset :disabled="!form.override_memory" :class="{ 'opacity-50': !form.override_memory }">
            <UFormField :label="$t('instSettings.memory')" :description="$t('instSettings.memoryDesc')">
              <div class="flex items-center gap-3 pt-1">
                <USlider v-model="memoryMb" :min="sysMem.minMb" :max="sysMem.maxMb.value" :step="256" class="flex-1" />
                <span class="w-24 shrink-0 text-right font-mono text-sm">{{ memoryMb }} MB</span>
              </div>
            </UFormField>
          </fieldset>
        </div>

        <div class="space-y-2">
          <USwitch v-model="form.override_java_args" :label="$t('instSettings.customJavaArgs')" :description="$t('instSettings.customJavaArgsDesc')" />
          <fieldset :disabled="!form.override_java_args" :class="{ 'opacity-50': !form.override_java_args }" class="space-y-2">
            <div class="flex items-center gap-2">
              <span class="shrink-0 text-xs text-muted">{{ $t('jvmPreset.label') }}</span>
              <div class="flex flex-wrap gap-1.5">
                <button
                  v-for="p in jvmPresets"
                  :key="p.key"
                  type="button"
                  class="rounded-md border px-2.5 py-1 text-xs font-medium transition"
                  :class="activePreset === p.key
                    ? 'border-primary-500/60 bg-primary-500/15 text-primary-300'
                    : 'border-default bg-white/3 text-neutral-400 hover:border-white/20 hover:text-neutral-200'"
                  @click="applyPreset(p.key)"
                >
                  {{ $t(p.label) }}
                </button>
              </div>
            </div>
            <UTextarea v-model="javaArgsText" :rows="3" placeholder="-Xmx4G -XX:+UseG1GC" class="w-full font-mono text-xs" />
          </fieldset>
        </div>

        <div class="space-y-2">
          <USwitch v-model="form.override_env" :label="$t('instSettings.customEnv')" :description="$t('instSettings.customEnvDesc')" />
          <fieldset :disabled="!form.override_env" class="space-y-2" :class="{ 'opacity-50': !form.override_env }">
            <div v-for="(e, i) in form.env_vars" :key="i" class="flex gap-2">
              <UInput v-model="e.key" placeholder="KEY" class="flex-1" />
              <UInput v-model="e.value" placeholder="value" class="flex-1" />
              <UButton icon="i-lucide-x" color="neutral" variant="ghost" square @click="form.env_vars.splice(i, 1)" />
            </div>
            <UButton icon="i-lucide-plus" size="xs" variant="soft" :label="$t('instSettings.addEnv')" @click="form.env_vars.push({ key: '', value: '' })" />
            <p class="text-xs text-muted">{{ $t('instSettings.envNote') }}</p>
          </fieldset>
        </div>
      </template>

      <template v-else-if="section === 'hooks'">
        <USwitch v-model="form.override_hooks" :label="$t('instSettings.customHooks')" :description="$t('instSettings.customHooksDesc')" />
        <fieldset :disabled="!form.override_hooks" class="space-y-4" :class="{ 'opacity-50': !form.override_hooks }">
          <UFormField :label="$t('instSettings.preLaunch')" :description="$t('instSettings.preLaunchHint')">
            <UInput v-model="form.pre_launch" class="w-full font-mono text-xs" />
          </UFormField>
          <UFormField :label="$t('instSettings.wrapper')" :description="$t('instSettings.wrapperHint')">
            <UInput v-model="form.wrapper" class="w-full font-mono text-xs" />
          </UFormField>
          <UFormField :label="$t('instSettings.postExit')" :description="$t('instSettings.postExitHint')">
            <UInput v-model="form.post_exit" class="w-full font-mono text-xs" />
          </UFormField>
        </fieldset>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { Instance } from '~/types/launcher'

const props = defineProps<{ instanceId: string }>()
const emit = defineEmits<{ (e: 'icon-changed'): void }>()
const instances = useInstancesStore()
const instance = computed(() => instances.instances.find(i => i.id === props.instanceId))

const iconKey = ref(0)
const pickingIcon = ref(false)
const iconEditorOpen = ref(false)

const iconMenu = computed(() => [[
  {
    label: t('iconEditor.create'),
    icon: 'i-lucide-palette',
    onSelect: () => { iconEditorOpen.value = true },
  },
  {
    label: t('iconEditor.upload'),
    icon: 'i-lucide-upload',
    onSelect: changeIcon,
  },
]])

async function onIconChanged() {
  iconKey.value++
  await instances.load()
  emit('icon-changed')
}

async function changeIcon() {
  pickingIcon.value = true
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    })
    if (typeof picked !== 'string') return
    await invoke('set_instance_icon', { id: props.instanceId, sourcePath: picked })
    invalidateInstanceIcon(props.instanceId)
    await onIconChanged()
    toast.add({ title: t('instance.iconChanged'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    pickingIcon.value = false
  }
}
const changeLoaderModal = useChangeLoaderModal()
const router = useRouter()
const toast = useToast()
const sysMem = useSystemMemory()
const java = useJava()

onMounted(() => {
  sysMem.ensure()
  if (!java.installations.value.length) java.scan()
})

const requiredJava = computed(() => (form.value ? requiredJavaMajor(form.value.mc_version) : 21))
const matchedJava = computed(() => matchJava(java.installations.value, requiredJava.value))

type Section = 'general' | 'install' | 'window' | 'java' | 'hooks'
const sections: { key: Section; label: string; icon: string }[] = [
  { key: 'general', label: 'instSettings.tabs.general', icon: 'i-lucide-info' },
  { key: 'install', label: 'instSettings.tabs.install', icon: 'i-lucide-wrench' },
  { key: 'window', label: 'instSettings.tabs.window', icon: 'i-lucide-monitor' },
  { key: 'java', label: 'instSettings.tabs.java', icon: 'i-lucide-coffee' },
  { key: 'hooks', label: 'instSettings.tabs.hooks', icon: 'i-lucide-code' },
]
const section = ref<Section>('general')

const busy = ref(false)
const repairing = ref(false)

const form = ref<Instance | null>(null)

watch(
  () => props.instanceId,
  (id) => {
    const inst = instances.instances.find(i => i.id === id)
    form.value = inst ? JSON.parse(JSON.stringify(inst)) as Instance : null
  },
  { immediate: true },
)

const loaderVersion = computed(() =>
  form.value && 'version' in form.value.loader ? form.value.loader.version : '',
)

const memoryMb = computed({
  get: () => form.value?.memory_mb ?? 4096,
  set: (v: number) => { if (form.value) form.value.memory_mb = v },
})

const javaArgsText = computed({
  get: () => form.value?.java_args.join(' ') ?? '',
  set: (v: string) => { if (form.value) form.value.java_args = v.split(/\s+/).filter(Boolean) },
})

const JVM_PRESET_FLAGS: Record<string, string[]> = {
  none: [],
  aikar: [
    '-XX:+UseG1GC', '-XX:+ParallelRefProcEnabled', '-XX:MaxGCPauseMillis=200',
    '-XX:+UnlockExperimentalVMOptions', '-XX:+DisableExplicitGC',
    '-XX:+AlwaysPreTouch', '-XX:G1NewSizePercent=30', '-XX:G1MaxNewSizePercent=40',
    '-XX:G1HeapRegionSize=8M', '-XX:G1ReservePercent=20', '-XX:G1HeapWastePercent=5',
    '-XX:G1MixedGCCountTarget=4', '-XX:InitiatingHeapOccupancyPercent=15',
    '-XX:G1MixedGCLiveThresholdPercent=90', '-XX:G1RSetUpdatingPauseTimePercent=5',
    '-XX:SurvivorRatio=32', '-XX:+PerfDisableSharedMem', '-XX:MaxTenuringThreshold=1',
  ],
  zgc: ['-XX:+UseZGC', '-XX:+ZGenerational'],
  shenandoah: ['-XX:+UseShenandoahGC', '-XX:ShenandoahGCMode=iu'],
}

const jvmPresets = [
  { key: 'none', label: 'jvmPreset.none' },
  { key: 'aikar', label: 'jvmPreset.aikar' },
  { key: 'zgc', label: 'jvmPreset.zgc' },
  { key: 'shenandoah', label: 'jvmPreset.shenandoah' },
]

const activePreset = computed(() => {
  const current = form.value?.java_args ?? []
  if (current.length === 0) return 'none'
  for (const [key, flags] of Object.entries(JVM_PRESET_FLAGS)) {
    if (key === 'none') continue
    if (flags.length === current.length && flags.every((f, i) => f === current[i])) return key
  }
  return 'custom'
})

function applyPreset(key: string) {
  if (!form.value) return
  form.value.java_args = JVM_PRESET_FLAGS[key] ?? []
}

let debounce: ReturnType<typeof setTimeout> | undefined
watch(form, () => {
  if (!form.value) return
  clearTimeout(debounce)
  const snapshot = JSON.parse(JSON.stringify(form.value)) as Instance
  debounce = setTimeout(() => instances.update(snapshot).catch((e: unknown) => toast.add({ title: String(e), color: 'error' })), 500)
}, { deep: true })

async function browseJava() {
  const p = await open({ multiple: false, directory: false })
  if (typeof p === 'string' && form.value) form.value.java_path = p
}

async function duplicate() {
  busy.value = true
  try {
    const inst = await invoke<Instance>('duplicate_instance', { id: props.instanceId })
    await instances.load()
    router.push(`/instance/${inst.id}`)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    busy.value = false
  }
}

async function remove() {
  try {
    await instances.remove(props.instanceId)
    router.push('/')
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function repair() {
  repairing.value = true
  try {
    await invoke('repair_instance', { id: props.instanceId })
    toast.add({ title: t('instSettings.repaired'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    repairing.value = false
  }
}

const { t } = useI18n()
</script>
