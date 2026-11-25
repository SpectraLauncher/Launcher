<template>
  <div
    class="flex h-16 w-full overflow-hidden ring-1 ring-black/40"
    :class="square ? '' : 'rounded-md'"
    :style="{ background: c.chrome }"
    aria-hidden="true"
  >
    <div class="flex w-5 flex-col items-center gap-1 pt-2">
      <span class="size-2.5" :class="square ? '' : 'rounded-[3px]'" :style="{ background: 'var(--ui-primary)' }" />
      <span class="size-2.5" :class="square ? '' : 'rounded-[3px]'" :style="{ background: c.line }" />
      <span class="size-2.5" :class="square ? '' : 'rounded-[3px]'" :style="{ background: c.line }" />
    </div>
    <div class="mt-1.5 flex-1 border-l border-t p-2" :class="square ? '' : 'rounded-tl-md'" :style="{ background: c.bg, borderColor: c.line }">
      <div class="flex items-center gap-1.5 border p-1.5" :class="square ? '' : 'rounded'" :style="{ background: c.card, borderColor: c.line }">
        <span class="size-3.5" :class="square ? '' : 'rounded-[3px]'" :style="{ background: c.line }" />
        <span class="h-1.5 flex-1" :class="square ? '' : 'rounded-full'" :style="{ background: c.line }" />
        <span class="h-3.5 w-5" :class="square ? '' : 'rounded-[3px]'" :style="{ background: 'var(--ui-primary)' }" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ThemeMode } from '~/stores/useThemeStore'

const props = defineProps<{ mode: ThemeMode }>()

const PALETTES = {
  dark: { chrome: '#0f1013', bg: '#16181c', card: '#1c1e23', line: '#2b2f36' },
  oled: { chrome: '#000000', bg: '#000000', card: '#0a0a0b', line: '#1f2023' },
}

const c = computed(() => (props.mode === 'oled' ? PALETTES.oled : PALETTES.dark))
const square = computed(() => props.mode === 'squared')
</script>
