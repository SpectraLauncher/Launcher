<template>
  <iframe
    ref="frame"
    :src="src"
    :title="title"
    sandbox="allow-scripts"
    referrerpolicy="no-referrer"
    class="block border-0 bg-transparent"
  />
</template>

<script setup lang="ts">
import type { AddonFrameHandle } from '~/composables/useAddonBridge'

const props = withDefaults(defineProps<{
  addonId: string
  entry: string
  context?: Record<string, string>
  title?: string
}>(), {
  context: () => ({}),
  title: '',
})

const addons = useAddonsStore()
const frame = ref<HTMLIFrameElement | null>(null)
const src = computed(() => addons.fileUrl(props.addonId, props.entry, props.context))

let handle: AddonFrameHandle | null = null

function register() {
  if (handle) unregisterAddonFrame(handle)
  handle = null
  const win = frame.value?.contentWindow
  if (win) handle = registerAddonFrame(props.addonId, win)
}

watch(() => props.addonId, register)
onMounted(register)
onBeforeUnmount(() => {
  if (handle) unregisterAddonFrame(handle)
  handle = null
})
</script>
