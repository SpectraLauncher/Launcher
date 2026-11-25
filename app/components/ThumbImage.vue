<template>
  <img v-if="src" :src="src" loading="lazy" :alt="alt" >
  <div v-else class="animate-pulse bg-elevated" />
</template>

<script setup lang="ts">
const props = withDefaults(defineProps<{
  path: string | null | undefined
  size?: number
  alt?: string
}>(), { size: 440, alt: '' })

const src = useThumbnail(() => props.path, () => props.size)
</script>
