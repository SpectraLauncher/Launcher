<template>
  <div class="h-full">
    <AddonFrame
      v-if="view"
      :key="`${addonId}:${view.id}`"
      :addon-id="addonId"
      :entry="view.entry"
      :title="view.title"
      class="h-full w-full"
    />
    <div v-else class="flex h-full items-center justify-center text-sm text-muted">
      {{ $t('addons.pageMissing') }}
    </div>
  </div>
</template>

<script setup lang="ts">
const route = useRoute()
const addons = useAddonsStore()

const addonId = computed(() => String(route.params.id ?? ''))
const view = computed(() =>
  addons.active.find(a => a.id === addonId.value)?.pages.find(p => p.id === String(route.params.page ?? '')) ?? null)
</script>
