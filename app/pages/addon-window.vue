<template>
  <div class="h-full">
    <AddonFrame
      v-if="win"
      :key="`${addonId}:${win.id}`"
      :addon-id="addonId"
      :entry="win.entry"
      :title="win.title"
      class="h-full w-full"
    />
    <div v-else class="flex h-full items-center justify-center text-sm text-muted">
      {{ $t('addons.pageMissing') }}
    </div>
  </div>
</template>

<script setup lang="ts">
definePageMeta({ layout: 'browser' })

const route = useRoute()
const addons = useAddonsStore()

const addonId = computed(() => String(route.query.addon ?? ''))
const win = computed(() =>
  addons.active.find(a => a.id === addonId.value)?.windows.find(w => w.id === String(route.query.window ?? '')) ?? null)
</script>
