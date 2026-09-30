<template>
  <div class="hidden" aria-hidden="true">
    <AddonFrame
      v-for="a in withMain"
      :key="`${a.id}@${a.version}`"
      :addon-id="a.id"
      entry="__spectra/main.html"
    />
  </div>
</template>

<script setup lang="ts">
const addons = useAddonsStore()
const withMain = computed(() => addons.active.filter(a => a.main))
</script>
