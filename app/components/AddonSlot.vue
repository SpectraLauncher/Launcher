<template>
  <template v-for="b in buttons" :key="`${b.addonId}:${b.id}`">
    <button
      v-if="variant === 'sidebar'"
      type="button"
      class="flex items-center justify-start gap-2 relative py-1 px-3 rounded-lg overflow-hidden text-left duration-300 hover:bg-primary-500/5"
      @click="click(b)"
    >
      <img v-if="b.icon" :src="convertFileSrc(b.icon)" alt="" class="size-[18px] shrink-0 object-contain">
      <UIcon v-else name="i-lucide-puzzle" class="size-[18px] shrink-0" />
      <p class="truncate">{{ label(b) }}</p>
    </button>
    <UButton
      v-else-if="variant === 'icon'"
      color="neutral"
      variant="ghost"
      size="sm"
      square
      :title="label(b)"
      :aria-label="label(b)"
      @click="click(b)"
    >
      <img v-if="b.icon" :src="convertFileSrc(b.icon)" alt="" class="size-4 object-contain">
      <UIcon v-else name="i-lucide-puzzle" class="size-4" />
    </UButton>
    <UButton
      v-else
      color="neutral"
      variant="soft"
      :label="label(b)"
      @click="click(b)"
    >
      <template #leading>
        <img v-if="b.icon" :src="convertFileSrc(b.icon)" alt="" class="size-4 object-contain">
        <UIcon v-else name="i-lucide-puzzle" class="size-4" />
      </template>
    </UButton>
  </template>
</template>

<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import type { SlotButton } from '~/stores/useAddonsStore'

const props = withDefaults(defineProps<{
  name: string
  variant?: 'sidebar' | 'icon' | 'button'
  context?: Record<string, string>
}>(), {
  variant: 'button',
  context: () => ({}),
})

const addons = useAddonsStore()
const toast = useToast()
const { t, locale } = useI18n()

async function click(b: SlotButton) {
  try {
    if (!await runAddonButton(b, props.context)) toast.add({ title: t('addons.commandMissing'), color: 'warning' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

const buttons = computed(() => addons.buttons(props.name))
const label = (b: SlotButton) => addons.text(b.addonId, b.title, locale.value)
</script>
