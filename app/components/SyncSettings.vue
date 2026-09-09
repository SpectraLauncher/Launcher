<template>
  <div class="space-y-6">
    <div>
      <p class="text-sm font-medium">{{ $t('sync.title') }}</p>
      <p class="mt-1 text-xs text-muted">{{ $t('sync.desc') }}</p>
    </div>

    <div class="space-y-1">
      <div
        v-for="option in SYNC_OPTIONS"
        :key="option"
        class="flex items-start gap-3 rounded-lg px-3 py-2.5 transition hover:bg-white/3"
      >
        <UIcon :name="OPTION_ICONS[option]" class="mt-0.5 size-4 shrink-0 text-neutral-500" />
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">{{ $t(`sync.options.${option}.label`) }}</p>
          <p class="mt-0.5 text-xs text-muted">{{ $t(`sync.options.${option}.desc`) }}</p>
          <p v-if="seededFrom(option)" class="mt-1 font-mono text-[11px] text-neutral-500">
            {{ $t('sync.seededFrom', { version: seededFrom(option) }) }}
          </p>
        </div>
        <USwitch
          :model-value="sync.state.value.global[option]"
          :loading="sync.busy.value === option"
          @update:model-value="toggle(option, $event)"
        />
      </div>
    </div>

    <div v-if="activeOptions.length" class="space-y-2">
      <div>
        <p class="text-sm font-medium">{{ $t('sync.instancesTitle') }}</p>
        <p class="mt-1 text-xs text-muted">{{ $t('sync.instancesDesc') }}</p>
      </div>

      <div class="overflow-x-auto rounded-lg border border-default">
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b border-default">
              <th class="px-3 py-2 text-left text-xs font-medium text-muted">
                {{ $t('sync.instanceColumn') }}
              </th>
              <th
                v-for="option in activeOptions"
                :key="option"
                class="px-2 py-2 text-center"
                :title="$t(`sync.options.${option}.label`)"
              >
                <UIcon :name="OPTION_ICONS[option]" class="size-4 text-neutral-400" />
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="source in sources"
              :key="source.id"
              class="border-b border-default/50 last:border-0 hover:bg-white/3"
            >
              <td class="px-3 py-2">
                <span class="block truncate font-medium">{{ source.name }}</span>
                <span class="font-mono text-[11px] text-neutral-500">{{ source.mc_version }}</span>
              </td>
              <td v-for="option in activeOptions" :key="option" class="px-2 py-2 text-center">
                <UCheckbox
                  :model-value="sync.participates(source.id, option)"
                  @update:model-value="toggleInstance(source.id, option, $event === true)"
                />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-if="!sources.length" class="text-xs text-muted">{{ $t('sync.sourceEmpty') }}</p>
    </div>

    <div v-if="sync.state.value.global.resource_packs" class="space-y-2">
      <div class="flex items-center justify-between">
        <p class="text-sm font-medium">{{ $t('sync.packsTitle') }}</p>
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :label="$t('content.refresh')"
          @click="refreshPacks"
        />
      </div>
      <p v-if="!sync.packs.value.length" class="text-xs text-muted">{{ $t('sync.packsEmpty') }}</p>
      <div
        v-for="pack in sync.packs.value"
        :key="pack.id"
        class="flex items-center gap-3 rounded-lg border border-default px-3 py-2"
      >
        <UCheckbox
          :model-value="pack.enabled"
          @update:model-value="setPackEnabled(pack.id, $event === true)"
        />
        <span class="min-w-0 flex-1 truncate text-sm" :title="pack.filename">{{ pack.filename }}</span>
        <span v-if="pack.pack_format !== null" class="shrink-0 font-mono text-[11px] text-neutral-500">
          {{ $t('sync.packFormat', { n: pack.pack_format }) }}
        </span>
        <UButton
          icon="i-lucide-trash-2"
          color="error"
          variant="ghost"
          size="xs"
          square
          :title="$t('common.remove')"
          @click="removePack(pack.id)"
        />
      </div>
    </div>

    <UButton
      icon="i-lucide-folder-open"
      color="neutral"
      variant="soft"
      size="xs"
      :label="$t('sync.openFolder')"
      @click="reveal"
    />

    <SyncSourceModal v-model:open="sourceOpen" :sources="sources" @picked="seed" />

    <UModal v-model:open="conflictOpen" :title="$t('sync.conflictTitle')">
      <template #body>
        <p class="text-sm text-muted">{{ $t('sync.conflictDesc') }}</p>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton
            color="neutral"
            variant="soft"
            :label="$t('sync.useInstance')"
            @click="resolveWith('use_instance')"
          />
          <UButton :label="$t('sync.useSynced')" @click="resolveWith('use_synced')" />
        </div>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { SYNC_OPTIONS, type JoinResolution, type SyncOption, type SyncSource } from '~/types/sync'

const OPTION_ICONS: Record<SyncOption, string> = {
  game_options: 'i-lucide-sliders-horizontal',
  multiplayer_servers: 'i-lucide-server',
  resource_packs: 'i-lucide-image',
  command_history: 'i-lucide-terminal',
  creative_hotbars: 'i-lucide-layout-grid',
}

const sync = useInstanceSync()
const toast = useToast()
const { t } = useI18n()

const sources = ref<SyncSource[]>([])
const sourceOpen = ref(false)
const pendingSeed = ref<SyncOption | null>(null)
const conflictOpen = ref(false)
const pendingJoin = ref<{ instanceId: string, option: SyncOption } | null>(null)

const activeOptions = computed(() => SYNC_OPTIONS.filter(o => sync.state.value.global[o]))
const seededFrom = (option: SyncOption) => sync.state.value.seeded_from[option] ?? null

onMounted(async () => {
  await run(() => sync.ensureLoaded())
  await loadSources()
})

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  try {
    return await fn()
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
    return undefined
  }
}

async function loadSources() {
  const list = await run(() => sync.sources())
  if (list) sources.value = list
}

async function toggle(option: SyncOption, enabled: boolean) {
  if (!enabled) {
    await run(() => sync.setGlobal(option, false))
    return
  }
  await loadSources()
  if (!sources.value.length) {
    toast.add({ title: t('sync.sourceEmpty'), color: 'error' })
    return
  }
  pendingSeed.value = option
  sourceOpen.value = true
}

async function seed(instanceId: string) {
  const option = pendingSeed.value
  pendingSeed.value = null
  if (!option) return
  const done = await run(() => sync.setGlobal(option, true, instanceId))
  if (done !== undefined) {
    toast.add({ title: t('sync.enabled', { option: t(`sync.options.${option}.label`) }) })
  }
}

async function toggleInstance(instanceId: string, option: SyncOption, enabled: boolean) {
  if (!enabled) {
    await run(() => sync.setInstance(instanceId, option, false))
    return
  }
  const action = await run(() => sync.joinPreview(instanceId, option))
  if (action === undefined) return
  if (action === 'requires_resolution') {
    pendingJoin.value = { instanceId, option }
    conflictOpen.value = true
    return
  }
  await run(() => sync.setInstance(instanceId, option, true))
}

async function resolveWith(resolution: JoinResolution) {
  const join = pendingJoin.value
  conflictOpen.value = false
  pendingJoin.value = null
  if (!join) return
  await run(() => sync.setInstance(join.instanceId, join.option, true, resolution))
}

function refreshPacks() {
  void run(() => sync.loadPacks())
}

function setPackEnabled(packId: string, enabled: boolean) {
  void run(() => sync.setPackEnabled(packId, enabled))
}

function removePack(packId: string) {
  void run(() => sync.removePack(packId))
}

function reveal() {
  void run(() => sync.openFolder())
}
</script>
