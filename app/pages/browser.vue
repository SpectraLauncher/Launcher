<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { ContentWindowConfig } from '~/composables/useContentWindow'
import type { Instance } from '~/types/launcher'

definePageMeta({ layout: 'browser' })

const contentWindow = useContentWindow()

const config = ref<ContentWindowConfig | null>(null)

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  config.value = await invoke<ContentWindowConfig | null>('content_window_config').catch(() => null)
  unlisten = await listen<ContentWindowConfig>('content://config', (e) => {
    config.value = e.payload
  })
})

onBeforeUnmount(() => unlisten?.())

function onInstalled(instance?: Instance) {
  contentWindow.announce({ instanceId: config.value?.instanceId, instance })
  if (instance) contentWindow.close()
}
</script>

<template>
  <div class="h-full">
    <ModrinthBrowser
      v-if="config"
      :config="config"
      @installed="onInstalled"
      @close="contentWindow.close()"
    />
  </div>
</template>
