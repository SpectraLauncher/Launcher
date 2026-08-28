<template>
  <div class="h-full overflow-y-auto p-6 lg:p-8">
    <div class="mx-auto max-w-5xl space-y-6">
      <div class="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ $t('worldsPage.title') }}</h1>
          <p class="mt-1 text-sm text-muted">
            {{ $t('worldsPage.count', { worlds: totalWorlds, instances: groups.length }) }}
          </p>
        </div>
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :loading="loading"
          :label="$t('content.refresh')"
          @click="load"
        />
      </div>

      <div v-if="loading" class="space-y-8">
        <div v-for="g in 2" :key="`g-sk-${g}`" class="space-y-3">
          <div class="flex items-center gap-2.5">
            <div class="size-8 animate-pulse rounded-lg bg-white/5" />
            <div class="h-4 w-40 animate-pulse rounded bg-white/5" />
          </div>
          <div class="space-y-2">
            <div v-for="n in 3" :key="`r-sk-${n}`" class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3">
              <div class="size-11 shrink-0 animate-pulse rounded-lg bg-white/5" />
              <div class="min-w-0 flex-1 space-y-2">
                <div class="h-3.5 w-1/3 animate-pulse rounded bg-white/5" />
                <div class="h-2.5 w-1/2 animate-pulse rounded bg-white/5" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="!groups.length" class="flex flex-col items-center justify-center gap-3 py-24 text-center">
        <UIcon name="i-lucide-globe" class="size-12 text-neutral-600" />
        <p class="text-sm text-muted">{{ $t('worldsPage.empty') }}</p>
      </div>

      <template v-else>
        <section v-for="group in groups" :key="group.instance.id">
          <button
            type="button"
            class="group/h mb-3 flex w-full items-center gap-2.5 text-left"
            @click="router.push(`/instance/${group.instance.id}`)"
          >
            <InstanceIcon :instance="group.instance" class="size-8 rounded-lg text-[14px]" />
            <span class="font-semibold transition group-hover/h:text-primary-400">{{ group.instance.name }}</span>
            <span class="font-mono text-[11px] text-neutral-500">{{ group.instance.mc_version }}</span>
            <div class="h-px flex-1 bg-white/6" />
            <span class="text-xs text-neutral-500">{{ $t('worldsPage.instanceWorlds', { n: group.worlds.length }) }}</span>
          </button>

          <div class="space-y-2">
            <div
              v-for="w in group.worlds"
              :key="w.folder"
              class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3"
            >
              <img
                v-if="w.icon_path"
                :src="assetUrl(w.icon_path)"
                class="size-11 shrink-0 rounded-lg object-cover [image-rendering:pixelated]"
                :alt="w.name"
              >
              <div v-else class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-white/5">
                <UIcon name="i-lucide-globe" class="size-5 text-neutral-500" />
              </div>
              <div class="min-w-0 flex-1">
                <div class="truncate font-medium">{{ w.name }}</div>
                <div class="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-neutral-400">
                  <span v-if="w.version" class="font-mono">{{ w.version }}</span>
                  <span v-if="w.game_mode">· {{ $t(`content.gameMode.${w.game_mode}`) }}</span>
                  <span v-if="w.last_played">· {{ formatDate(w.last_played) }}</span>
                </div>
              </div>
              <UButton
                v-if="supportsQuickPlay(group.instance.mc_version)"
                icon="i-lucide-play"
                size="xs"
                color="primary"
                variant="ghost"
                :loading="launching === `${group.instance.id}/${w.folder}`"
                :title="$t('quickPlay.playWorld')"
                square
                @click="playWorld(group.instance.id, w.folder)"
              />
              <UButton
                icon="i-lucide-archive"
                size="xs"
                color="neutral"
                variant="ghost"
                :loading="busyWorld === `${group.instance.id}/${w.folder}`"
                :title="$t('content.backup')"
                square
                @click="backupWorld(group.instance.id, w)"
              />
              <UButton
                icon="i-lucide-trash-2"
                size="xs"
                color="error"
                variant="ghost"
                :title="$t('common.remove')"
                square
                @click="deleteWorld(group.instance.id, w)"
              />
            </div>
          </div>
        </section>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { save, confirm } from '@tauri-apps/plugin-dialog'
import type { Instance, WorldInfo } from '~/types/launcher'

const instances = useInstancesStore()
const router = useRouter()
const toast = useToast()
const { t } = useI18n()
const mc = useMinecraftLaunch()

const busyWorld = ref<string | null>(null)

interface WorldGroup { instance: Instance; worlds: WorldInfo[] }
const groups = ref<WorldGroup[]>([])
const loading = ref(true)
const launching = ref<string | null>(null)

const totalWorlds = computed(() => groups.value.reduce((n, g) => n + g.worlds.length, 0))

const assetUrl = (path: string) => convertFileSrc(path)
const formatDate = (ms: number) => new Date(ms).toLocaleDateString()

function supportsQuickPlay(ver: string): boolean {
  const [, minorStr = '0'] = ver.split('.')
  return parseInt(minorStr) >= 20
}

async function load() {
  loading.value = true
  try {
    await instances.ensureLoaded()
    const settled = await Promise.all(
      instances.instances.map(async (instance: Instance) => {
        try {
          const worlds = await invoke<WorldInfo[]>('list_worlds', { id: instance.id })
          return { instance, worlds }
        } catch {
          return { instance, worlds: [] as WorldInfo[] }
        }
      }),
    )
    groups.value = settled.filter((g: WorldGroup) => g.worlds.length > 0)
  } finally {
    loading.value = false
  }
}

async function backupWorld(instanceId: string, w: WorldInfo) {
  const dest = await save({ defaultPath: `${w.folder}.zip`, filters: [{ name: 'Zip', extensions: ['zip'] }] })
  if (typeof dest !== 'string') return
  busyWorld.value = `${instanceId}/${w.folder}`
  try {
    await invoke('backup_world', { id: instanceId, folder: w.folder, dest })
    toast.add({ title: t('content.backupDone'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    busyWorld.value = null
  }
}

async function deleteWorld(instanceId: string, w: WorldInfo) {
  const ok = await confirm(t('content.deleteWorldConfirm', { name: w.name }), { title: t('content.deleteWorldTitle'), kind: 'warning' })
  if (!ok) return
  try {
    await invoke('delete_world', { id: instanceId, folder: w.folder })
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function playWorld(instanceId: string, folder: string) {
  launching.value = `${instanceId}/${folder}`
  await router.push(`/instance/${instanceId}`)
  mc.launch(instanceId, { kind: 'Singleplayer', world: folder }).catch(() => {  })
  launching.value = null
}

onMounted(load)
</script>
