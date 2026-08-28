<template>
  <div v-if="showControls" class="flex gap-1" :class="{ 'flex-row-reverse': isMac }" style="-webkit-app-region: no-drag">
    <button class="flex items-center justify-center p-1 bg-gray-800/20 hover:bg-gray-800 rounded-md h-6 w-6 duration-300 cursor-pointer" title="Minimize" @click="minimize">
      <svg width="8" height="1" viewBox="0 0 10 1">
        <rect width="10" height="1" fill="currentColor" />
      </svg>
    </button>

    <button class="flex items-center justify-center p-1 bg-gray-800/20 hover:bg-gray-800 rounded-md h-6 w-6 duration-300 cursor-pointer" :title="isMaximized ? 'Restore' : 'Maximize'" @click="toggleMaximize">
      <svg v-if="isMaximized" width="8" height="8" viewBox="0 0 10 10">
        <path
          fill="none"
          stroke="currentColor"
          stroke-width="1"
          d="M2.5,0.5 L9.5,0.5 L9.5,7.5 L7.5,7.5 L7.5,9.5 L0.5,9.5 L0.5,2.5 L2.5,2.5 Z M2.5,2.5 L7.5,2.5 L7.5,7.5"
        />
      </svg>
      <svg v-else width="8" height="8" viewBox="0 0 10 10">
        <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>

    <button class="flex items-center justify-center p-1 bg-gray-800/20 hover:bg-gray-800 rounded-md h-6 w-6 duration-300 cursor-pointer close" title="Close" @click="close">
      <svg width="8" height="8" viewBox="0 0 10 10">
        <path fill="none" stroke="currentColor" stroke-width="1.2" d="M1,1 L9,9 M9,1 L1,9" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'

const { platform, ready } = usePlatform()

const isMaximized = ref(false)
const appWindow = getCurrentWindow()

const showControls = computed(() => platform.value === 'windows' || platform.value === 'linux' || platform.value === 'macos')
const isMac = computed(() => platform.value === 'macos')

const minimize = async () => { await appWindow.minimize() }

const toggleMaximize = async () => {
  if (isMaximized.value) {
    await appWindow.unmaximize()
  } else {
    await appWindow.maximize()
  }
}

const close = async () => { await appWindow.close() }

let unlistenResize: (() => void) | null = null

onMounted(async () => {
  await ready

  isMaximized.value = await appWindow.isMaximized()
  unlistenResize = await appWindow.onResized(async () => {
    isMaximized.value = await appWindow.isMaximized()
  })
})

onUnmounted(() => {
  if (unlistenResize) unlistenResize()
})
</script>
