<template>
  <UModal v-model:open="open" :dismissible="false" :ui="{ content: 'max-w-3xl' }">
    <template #content>
      <div class="grid grid-cols-1 sm:grid-cols-[1fr_320px]">
        <div class="flex flex-col gap-4 p-7">
          <span class="w-fit rounded-full bg-primary-500/15 px-3 py-1 text-xs font-semibold text-primary-400">
            {{ $t('syncAnnounce.badge') }} · 0.9.0
          </span>

          <div class="flex items-center gap-3">
            <UIcon name="i-lucide-cloud" class="size-7 shrink-0 text-primary-400" />
            <h2 class="text-2xl font-bold tracking-tight">{{ $t('syncAnnounce.title') }}</h2>
          </div>

          <p class="text-sm leading-relaxed text-muted">{{ $t('syncAnnounce.body') }}</p>
          <p class="text-sm leading-relaxed text-muted">{{ $t('syncAnnounce.conflicts') }}</p>
          <p class="text-sm leading-relaxed text-muted">{{ $t('syncAnnounce.later') }}</p>

          <p v-if="error" role="alert" class="text-xs text-red-400">{{ error }}</p>

          <div class="mt-auto flex flex-wrap gap-2 pt-2">
            <UButton
              icon="i-lucide-clock-3"
              color="neutral"
              variant="soft"
              :label="$t('syncAnnounce.skip')"
              :disabled="busy"
              @click="skip"
            />
            <UButton
              :icon="primaryIcon"
              :label="primaryLabel"
              :loading="busy"
              @click="primary"
            />
          </div>
        </div>

        <div class="flex flex-col gap-3 border-t border-default bg-white/2 p-5 sm:border-t-0 sm:border-l">
          <div class="flex justify-end">
            <UButton
              icon="i-lucide-x"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              :title="$t('common.close')"
              :disabled="busy"
              @click="skip"
            />
          </div>

          <p class="text-xs font-semibold uppercase tracking-wide text-muted">{{ $t('syncAnnounce.included') }}</p>
          <div class="space-y-2">
            <div class="flex items-center gap-2 text-sm">
              <UIcon name="i-lucide-list" class="size-4 shrink-0 text-primary-400" />
              <span>{{ $t('syncAnnounce.instances') }}</span>
            </div>
            <div v-for="option in SYNC_OPTIONS" :key="option" class="flex items-center gap-2 text-sm">
              <UIcon :name="OPTION_ICONS[option]" class="size-4 shrink-0 text-primary-400" />
              <span>{{ $t(`sync.options.${option}.label`) }}</span>
            </div>
          </div>

          <p class="mt-2 border-t border-default pt-3 text-xs leading-relaxed text-muted">
            {{ $t('syncAnnounce.localOnly') }}
          </p>
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { CloudSyncView } from '~/composables/useCloudSync'
import { SYNC_OPTIONS, type SyncOption } from '~/types/sync'

const OPTION_ICONS: Record<SyncOption, string> = {
  game_options: 'i-lucide-sliders-horizontal',
  multiplayer_servers: 'i-lucide-server',
  resource_packs: 'i-lucide-image',
  command_history: 'i-lucide-terminal',
  creative_hotbars: 'i-lucide-layout-grid',
}

const { t } = useI18n()
const updater = useAutoUpdate()
const spectra = useSpectraAccount()
const cloud = useCloudSync()
const router = useRouter()
const toast = useToast()

const open = useState('cloud-sync-announcement-open', () => false)
const pending = useState('cloud-sync-announcement-pending', () => true)
const busy = ref(false)
const error = ref<string | null>(null)
const alreadyEnabled = computed(() => cloud.view.value.enabled
  && cloud.view.value.accountId === spectra.user.value?.id)
const primaryLabel = computed(() => alreadyEnabled.value
  ? t('syncAnnounce.manage')
  : spectra.isSignedIn.value ? t('syncAnnounce.enable') : t('syncAnnounce.signIn'))
const primaryIcon = computed(() => alreadyEnabled.value
  ? 'i-lucide-settings-2'
  : spectra.isSignedIn.value ? 'i-lucide-cloud-upload' : 'i-lucide-log-in')

onMounted(async () => {
  // An update may restart the launcher. Offer the new feature only after that
  // check has finished, so the announcement survives an interrupted update.
  try {
    await updater.updateOnStartup()
    if (!await invoke<boolean>('take_cloud_sync_announcement')) return
    await Promise.allSettled([
      spectra.refresh(),
      invoke<CloudSyncView>('cloud_sync_state').then(state => { cloud.view.value = state }),
    ])
    open.value = true
  } catch (e) {
    console.error('cloud sync announcement check failed', e)
  } finally {
    pending.value = false
  }
})

async function finish() {
  await invoke('mark_cloud_sync_announcement_seen')
  open.value = false
}

async function skip() {
  if (busy.value) return
  busy.value = true
  error.value = null
  try {
    await finish()
  } catch (e) {
    error.value = errorText(e)
  } finally {
    busy.value = false
  }
}

async function primary() {
  if (busy.value) return
  busy.value = true
  error.value = null
  try {
    if (!spectra.isSignedIn.value) {
      await spectra.login()
      return
    }
    if (alreadyEnabled.value) {
      await finish()
      await router.push({ path: '/settings', query: { section: 'sync' } })
      return
    }
    try {
      await cloud.setEnabled(true)
    } catch (e) {
      if (!cloud.view.value.enabled) throw e
      toast.add({ title: t('syncAnnounce.retry'), description: errorText(e), color: 'warning' })
    }
    await finish()
    toast.add({ title: t('syncAnnounce.done') })
  } catch (e) {
    error.value = errorText(e)
  } finally {
    busy.value = false
  }
}
</script>
