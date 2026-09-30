<template>
  <UModal v-model:open="open" :title="project?.title ?? $t('addons.browse')" :ui="{ content: 'max-w-2xl' }">
    <template #body>
      <div v-if="loading" class="space-y-3">
        <div class="flex gap-4">
          <div class="size-16 shrink-0 animate-pulse rounded-xl bg-white/5" />
          <div class="flex-1 space-y-2">
            <div class="h-3 w-3/4 animate-pulse rounded bg-white/5" />
            <div class="h-3 w-1/2 animate-pulse rounded bg-white/5" />
          </div>
        </div>
        <div class="h-32 animate-pulse rounded-xl bg-white/5" />
      </div>

      <p v-else-if="failed" class="py-6 text-center text-sm text-muted">{{ $t('addons.previewFailed') }}</p>

      <div v-else-if="project" class="space-y-5">
        <div class="flex items-start gap-4">
          <img v-if="project.icon" :src="project.icon" alt="" class="size-16 shrink-0 rounded-xl object-cover">
          <div v-else class="flex size-16 shrink-0 items-center justify-center rounded-xl bg-white/5">
            <UIcon name="i-lucide-puzzle" class="size-7" />
          </div>
          <div class="min-w-0 flex-1">
            <p class="text-sm text-muted">{{ project.summary }}</p>
            <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted">
              <span v-if="owner">{{ $t('addons.by', { name: owner }) }}</span>
              <span>{{ $t('addons.downloads', { n: count(project.downloads) }) }}</span>
              <span v-if="latest">{{ $t('addons.version', { version: latest.number }) }}</span>
              <span v-if="latest">
                {{ launcherRange ? $t('addons.worksWith', { range: launcherRange }) : $t('addons.anyLauncher') }}
              </span>
            </div>
          </div>
        </div>

        <div>
          <p class="mb-1.5 text-sm font-medium">{{ $t('addons.permissions') }}</p>
          <ul v-if="permissions.length" class="space-y-1 text-sm text-muted">
            <li v-for="p in permissions" :key="p" class="flex items-center gap-2">
              <UIcon :name="p.startsWith('network:') ? 'i-lucide-globe' : 'i-lucide-lock'" class="size-3.5 shrink-0" />
              {{ addonPermissionLabel(t, p) }}
            </li>
          </ul>
          <p v-else class="text-sm text-muted">{{ $t('addons.noPermissions') }}</p>
          <p v-if="runsCode" class="mt-2 text-xs text-dimmed">{{ $t('addons.runsCode') }}</p>
        </div>

        <div v-if="gallery.length">
          <p class="mb-1.5 text-sm font-medium">{{ $t('addons.gallery') }}</p>
          <img
            v-if="bigImage"
            :src="bigImage.url"
            :alt="bigImage.title"
            class="mb-2 w-full rounded-xl object-contain"
          >
          <div class="flex gap-2 overflow-x-auto pb-1">
            <button
              v-for="(g, i) in gallery"
              :key="g.url"
              type="button"
              class="shrink-0 overflow-hidden rounded-lg ring-2 transition"
              :class="big === i ? 'ring-primary-500' : 'ring-transparent hover:ring-white/20'"
              @click="big = big === i ? null : i"
            >
              <img :src="g.url" :alt="g.title" class="h-20 w-32 object-cover">
            </button>
          </div>
        </div>

        <div>
          <div v-if="bodyHtml" class="mk-md" v-html="bodyHtml" />
          <p v-else class="text-sm text-muted">{{ $t('addons.noDescription') }}</p>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton
          color="neutral"
          variant="ghost"
          icon="i-lucide-external-link"
          :label="$t('addons.openOnSite')"
          @click="openSite"
        />
        <UButton
          icon="i-lucide-download"
          :disabled="!project"
          :label="installed ? $t('addons.update') : $t('addons.install')"
          @click="emit('install', slug!)"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'

interface PreviewVersion {
  number: string
  meta: Record<string, unknown>
}

interface PreviewProject {
  slug: string
  path: string
  title: string
  summary: string
  description: string
  icon: string | null
  downloads: number
  owner: { name: string | null, slug: string | null } | null
  versions: PreviewVersion[]
}

const slug = defineModel<string | null>('slug', { required: true })
defineProps<{ installed: boolean }>()
const emit = defineEmits<{ install: [slug: string] }>()

const { t, locale } = useI18n()
const { bodyHtml, renderBody } = useProjectDetailView()

const open = computed({
  get: () => slug.value !== null,
  set: (value: boolean) => { if (!value) slug.value = null },
})

const project = ref<PreviewProject | null>(null)
const gallery = ref<Array<{ url: string, title: string }>>([])
const loading = ref(false)
const failed = ref(false)
const big = ref<number | null>(null)

const bigImage = computed(() => (big.value === null ? null : gallery.value[big.value] ?? null))
const latest = computed(() => project.value?.versions[0] ?? null)
const owner = computed(() => project.value?.owner?.name || project.value?.owner?.slug || '')
const launcherRange = computed(() => {
  const range = latest.value?.meta.launcher
  return typeof range === 'string' && range.trim() ? range.trim() : ''
})
const permissions = computed(() => {
  const list = latest.value?.meta.permissions
  return Array.isArray(list) ? list.filter((p): p is string => typeof p === 'string') : []
})
const runsCode = computed(() => {
  const meta = latest.value?.meta ?? {}
  const contributes = (meta.contributes ?? {}) as Record<string, unknown>
  const some = (value: unknown) => Array.isArray(value) && value.length > 0
  return Boolean(meta.main) || Boolean(meta.backend) || Boolean(contributes.settings)
    || some(contributes.pages) || some(contributes.instanceTabs) || some(contributes.windows)
})

const count = (n: number) => new Intl.NumberFormat(locale.value).format(n)

watch(slug, async (value) => {
  project.value = null
  gallery.value = []
  big.value = null
  failed.value = false
  if (!value) return
  loading.value = true
  try {
    const res = await invoke<{ project: PreviewProject, gallery?: Array<{ url: string, title: string }> }>(
      'addons_catalog_project', { slug: value })
    if (slug.value !== value) return
    project.value = res.project
    gallery.value = res.gallery ?? []
    await renderBody(res.project.description ?? '')
  } catch {
    failed.value = true
  } finally {
    loading.value = false
  }
}, { immediate: true })

function openSite() {
  if (!slug.value) return
  void openExternal(`${SPECTRA_SITE}${project.value?.path ?? `/addon/${slug.value}`}`).catch(() => {})
}
</script>
