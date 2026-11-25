<template>
  <UTooltip :text="instance.name" :content="{ side: 'right' }">
    <button
      type="button"
      :aria-label="instance.name"
      :aria-current="active ? 'page' : undefined"
      class="relative shrink-0 rounded-xl p-0.5 ring-2 transition"
      :class="active ? 'ring-primary' : 'ring-transparent hover:ring-(--ui-border-accented)'"
      @click="onClick"
    >
      <InstanceIcon :instance="instance" class="size-9 rounded-[10px] text-[14px]" />
      <span
        v-if="running"
        class="absolute -bottom-0.5 -right-0.5 size-3 rounded-full border-2 border-(--mk-chrome) bg-primary"
      />
    </button>
  </UTooltip>
</template>

<script setup lang="ts">
import type { Instance } from '~/types/launcher'

const props = defineProps<{ instance: Instance }>()

const router = useRouter()
const route = useRoute()
const activity = useActivityCenter()

const active = computed(() => route.params.id === props.instance.id)
const running = computed(() => activity.list.value.some(a => a.kind === 'running' && a.instanceId === props.instance.id))

const onClick = () => {
  router.push(`/instance/${props.instance.id}`)
}
</script>
