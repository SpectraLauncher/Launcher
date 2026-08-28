<template>
  <div class="h-full overflow-y-auto p-6 lg:p-8">
    <div class="mx-auto max-w-6xl space-y-6">
      <div class="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold tracking-tight">{{ $t('screenshotsPage.title') }}</h1>
          <p class="mt-1 text-sm text-muted">
            {{ $t('screenshotsPage.count', { shots: totalShots, instances: groups.length }) }}
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
          <div class="grid gap-3" style="grid-template-columns:repeat(auto-fill,minmax(220px,1fr))">
            <div v-for="n in 4" :key="`s-sk-${n}`" class="overflow-hidden rounded-xl border border-default bg-white/3">
              <div class="aspect-video w-full animate-pulse bg-white/5" />
              <div class="px-2.5 py-1.5"><div class="h-3 w-2/3 animate-pulse rounded bg-white/5" /></div>
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="!groups.length" class="flex flex-col items-center justify-center gap-3 py-24 text-center">
        <UIcon name="i-lucide-camera" class="size-12 text-neutral-600" />
        <p class="text-sm text-muted">{{ $t('screenshotsPage.empty') }}</p>
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
            <span class="text-xs text-neutral-500">{{ $t('screenshotsPage.instanceShots', { n: group.shots.length }) }}</span>
          </button>

          <div class="grid gap-3" style="grid-template-columns:repeat(auto-fill,minmax(220px,1fr))">
            <button
              v-for="s in group.shots"
              :key="s.path"
              type="button"
              class="group overflow-hidden rounded-xl border border-default bg-white/3 text-left transition hover:border-primary-500/40"
              @click="openLightbox(s)"
            >
              <img :src="assetUrl(s.path)" loading="lazy" class="aspect-video w-full object-cover transition group-hover:opacity-90" :alt="s.name" >
              <div class="truncate px-2.5 py-1.5 text-[11px] text-neutral-400" :title="s.name">{{ s.name }}</div>
            </button>
          </div>
        </section>
      </template>
    </div>

    <UModal v-model:open="lightboxOpen" :ui="{ content: 'max-w-5xl w-[92vw]' }">
      <template #content>
        <div v-if="lightbox" class="flex flex-col">
          <div class="flex items-center justify-between gap-2 border-b border-default px-4 py-2.5">
            <span class="truncate text-sm font-medium" :title="lightbox.shot.name">
              {{ lightbox.shot.name }}
              <span class="ml-1 text-xs text-muted">{{ (lightboxIndex ?? 0) + 1 }} / {{ flatShots.length }}</span>
              <span class="ml-2 text-xs text-neutral-500">· {{ lightbox.instance.name }}</span>
            </span>
            <div class="flex items-center gap-1.5">
              <UButton icon="i-lucide-download" size="xs" color="neutral" variant="soft" :label="$t('content.download')" @click="downloadShot(lightbox.shot)" />
              <UButton icon="i-lucide-folder-open" size="xs" color="neutral" variant="soft" :label="$t('content.openLocation')" @click="revealShot(lightbox.shot)" />
              <UButton icon="i-lucide-trash-2" size="xs" color="error" variant="soft" :label="$t('common.remove')" @click="deleteShot(lightbox.instance.id, lightbox.shot)" />
              <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" square @click="lightboxIndex = null" />
            </div>
          </div>

          <div class="relative bg-black/40">
            <img :src="assetUrl(lightbox.shot.path)" class="max-h-[78vh] w-full object-contain" :alt="lightbox.shot.name" >
            <UButton
              v-if="flatShots.length > 1"
              icon="i-lucide-chevron-left"
              color="neutral"
              class="absolute top-1/2 left-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
              @click="step(-1)"
            />
            <UButton
              v-if="flatShots.length > 1"
              icon="i-lucide-chevron-right"
              color="neutral"
              class="absolute top-1/2 right-2 -translate-y-1/2 rounded-full opacity-80 hover:opacity-100"
              @click="step(1)"
            />
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>

<script setup lang="ts">
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { save, confirm } from '@tauri-apps/plugin-dialog'
import type { Instance, ScreenshotInfo } from '~/types/launcher'

const instances = useInstancesStore()
const router = useRouter()
const toast = useToast()
const { t } = useI18n()

interface ShotGroup { instance: Instance; shots: ScreenshotInfo[] }
const groups = ref<ShotGroup[]>([])
const loading = ref(true)

const totalShots = computed(() => groups.value.reduce((n, g) => n + g.shots.length, 0))
const flatShots = computed(() =>
  groups.value.flatMap(g => g.shots.map(shot => ({ shot, instance: g.instance }))),
)

const assetUrl = (path: string) => convertFileSrc(path)

async function load() {
  loading.value = true
  try {
    await instances.ensureLoaded()
    const settled = await Promise.all(
      instances.instances.map(async (instance: Instance) => {
        try {
          const shots = await invoke<ScreenshotInfo[]>('list_screenshots', { id: instance.id })
          return { instance, shots }
        } catch {
          return { instance, shots: [] as ScreenshotInfo[] }
        }
      }),
    )
    groups.value = settled.filter((g: ShotGroup) => g.shots.length > 0)
  } finally {
    loading.value = false
  }
}

const lightboxIndex = ref<number | null>(null)
const lightbox = computed(() => (lightboxIndex.value !== null ? flatShots.value[lightboxIndex.value] ?? null : null))
const lightboxOpen = computed({
  get: () => lightboxIndex.value !== null,
  set: (v: boolean) => { if (!v) lightboxIndex.value = null },
})

function openLightbox(s: ScreenshotInfo) {
  lightboxIndex.value = flatShots.value.findIndex(f => f.shot.path === s.path)
}
function step(delta: number) {
  if (lightboxIndex.value === null || !flatShots.value.length) return
  const n = flatShots.value.length
  lightboxIndex.value = (lightboxIndex.value + delta + n) % n
}

function onKey(e: KeyboardEvent) {
  if (lightboxIndex.value === null) return
  if (e.key === 'ArrowRight') { e.preventDefault(); step(1) }
  else if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1) }
  else if (e.key === 'Escape') lightboxIndex.value = null
}

async function downloadShot(s: ScreenshotInfo) {
  try {
    const dest = await save({ defaultPath: s.name, filters: [{ name: 'PNG', extensions: ['png'] }] })
    if (!dest) return
    await invoke('copy_file', { from: s.path, to: dest })
    toast.add({ title: t('content.downloaded'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function revealShot(s: ScreenshotInfo) {
  try {
    await invoke('reveal_in_explorer', { path: s.path })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function deleteShot(instanceId: string, s: ScreenshotInfo) {
  const ok = await confirm(t('content.deleteScreenshotConfirm'), { title: t('content.deleteScreenshotTitle'), kind: 'warning' })
  if (!ok) return
  try {
    await invoke('delete_screenshot', { id: instanceId, name: s.name })
    lightboxIndex.value = null
    await load()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

onMounted(() => {
  load()
  window.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>
