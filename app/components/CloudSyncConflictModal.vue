<template>
  <UModal :open="cloud.view.value.status === 'conflict' && cloud.view.value.accountId === spectra.user.value?.id && !announcementPending && !announcementOpen" :dismissible="false" :title="$t('sync.cloud.conflictTitle')">
    <template #body>
      <p class="text-sm text-muted">{{ $t('sync.cloud.conflictDesc') }}</p>
      <p v-if="cloud.error.value" role="alert" class="mt-3 text-xs text-red-400">{{ cloud.error.value }}</p>
    </template>
    <template #footer>
      <div class="flex w-full flex-wrap justify-end gap-2">
        <UButton
          color="neutral"
          variant="ghost"
          :loading="cloud.busy.value"
          :label="$t('sync.cloud.turnOff')"
          @click="turnOff"
        />
        <UButton
          color="neutral"
          variant="soft"
          :loading="cloud.busy.value"
          :label="$t('sync.cloud.useCloud')"
          @click="choose('cloud')"
        />
        <UButton
          :loading="cloud.busy.value"
          :label="$t('sync.cloud.useComputer')"
          @click="choose('computer')"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
const cloud = useCloudSync()
const spectra = useSpectraAccount()
const announcementOpen = useState('cloud-sync-announcement-open', () => false)
const announcementPending = useState('cloud-sync-announcement-pending', () => true)

function choose(choice: 'cloud' | 'computer') {
  void cloud.resolve(choice).catch(() => {})
}

function turnOff() {
  void cloud.setEnabled(false).catch(() => {})
}
</script>
