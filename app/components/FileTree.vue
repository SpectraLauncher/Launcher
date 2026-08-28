<template>
  <div>
    <div v-if="loading" class="py-2 pl-2 text-xs text-muted" :style="indent">{{ $t('common.loading') }}</div>
    <template v-else>
      <div v-for="child in children" :key="child.name">
        <div
          class="flex items-center gap-1.5 rounded-md py-1 pr-2 hover:bg-white/5"
          :style="indent"
        >
          <button
            v-if="child.is_dir"
            type="button"
            class="flex size-4 shrink-0 items-center justify-center text-neutral-500 hover:text-neutral-300"
            @click="toggleExpand(child.name)"
          >
            <UIcon :name="expanded[child.name] ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'" class="size-3.5" />
          </button>
          <span v-else class="size-4 shrink-0" />

          <button
            type="button"
            class="flex size-4 shrink-0 items-center justify-center rounded border transition"
            :class="checkboxClass(child)"
            @click="toggle(childPath(child.name), child.is_dir)"
          >
            <UIcon v-if="state(child) === 'checked'" name="i-lucide-check" class="size-3 text-white" />
            <UIcon v-else-if="state(child) === 'indeterminate'" name="i-lucide-minus" class="size-3 text-white" />
          </button>

          <UIcon :name="child.is_dir ? 'i-lucide-folder' : 'i-lucide-file'" class="size-4 shrink-0 text-neutral-500" />
          <span class="min-w-0 flex-1 truncate text-sm" :class="{ 'text-neutral-500 line-through': state(child) === 'unchecked' }">{{ child.name }}</span>
          <span v-if="!child.is_dir" class="shrink-0 font-mono text-[11px] text-muted">{{ fmtSize(child.size) }}</span>
        </div>

        <FileTree
          v-if="child.is_dir && expanded[child.name]"
          :instance-id="instanceId"
          :dir="childPath(child.name)"
          :excluded="excluded"
          :included="included"
          :toggle="toggle"
          :level="level + 1"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { DirChild } from '~/types/launcher'

const props = withDefaults(defineProps<{
  instanceId: string
  dir?: string
  excluded: Set<string>
  included: Set<string>
  toggle: (path: string, isDir: boolean) => void
  level?: number
}>(), { dir: '', level: 0 })

const children = ref<DirChild[]>([])
const loading = ref(false)
const expanded = ref<Record<string, boolean>>({})

const indent = computed(() => ({ paddingLeft: `${props.level * 16 + 4}px` }))
const childPath = (name: string) => (props.dir ? `${props.dir}/${name}` : name)

function fmtSize(n: number) {
  if (n < 1024) return `${n} B`
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(0)} KB`
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toFixed(1)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function isIncluded(path: string): boolean {
  const parts = path.split('/')
  for (let i = parts.length; i >= 1; i--) {
    const p = parts.slice(0, i).join('/')
    if (props.included.has(p)) return true
    if (props.excluded.has(p)) return false
  }
  return true
}
function hasMarkerUnder(path: string): boolean {
  const prefix = path + '/'
  for (const e of props.excluded) if (e.startsWith(prefix)) return true
  for (const e of props.included) if (e.startsWith(prefix)) return true
  return false
}

type State = 'checked' | 'unchecked' | 'indeterminate'
function state(c: DirChild): State {
  const path = childPath(c.name)
  if (c.is_dir && hasMarkerUnder(path)) return 'indeterminate'
  return isIncluded(path) ? 'checked' : 'unchecked'
}
function checkboxClass(c: DirChild) {
  return state(c) === 'unchecked' ? 'border-white/25 hover:border-white/40' : 'border-primary-500 bg-primary-500'
}

function toggleExpand(name: string) {
  expanded.value = { ...expanded.value, [name]: !expanded.value[name] }
}

async function load() {
  loading.value = true
  try {
    children.value = await invoke<DirChild[]>('list_dir', { id: props.instanceId, rel: props.dir })
  } catch {
    children.value = []
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>
