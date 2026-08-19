<template>
  <UModal v-model:open="open" :title="$t('iconEditor.title')" :ui="{ content: 'max-w-3xl' }">
    <template #body>
      <div class="grid gap-4 sm:grid-cols-[13rem_minmax(0,1fr)]">
        <!-- preview + randomize -->
        <div class="space-y-3">
          <div class="flex flex-col items-center gap-4 rounded-xl border border-default bg-white/3 p-4">
            <div class="size-28 overflow-hidden rounded-2xl" :style="tileStyle">
              <img :src="symbolUrl" alt="" class="size-full object-contain" :style="symbolStyle" />
            </div>
            <!-- the sizes it actually shows up in: card, sidebar, list row -->
            <div class="flex items-end gap-2">
              <div v-for="n in [40, 28, 20]" :key="n" class="overflow-hidden rounded-lg" :style="[tileStyle, { width: `${n}px`, height: `${n}px` }]">
                <img :src="symbolUrl" alt="" class="size-full object-contain" :style="symbolStyle" />
              </div>
            </div>
          </div>
          <UButton
            block
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="soft"
            :label="$t('iconEditor.randomize')"
            @click="randomize"
          />
        </div>

        <!-- pickers -->
        <div class="min-w-0 space-y-4">
          <section>
            <p class="mb-2 text-sm font-medium">{{ $t('iconEditor.background') }}</p>
            <div class="flex gap-2 overflow-x-auto py-2">
              <button
                v-for="(g, i) in GRADIENTS"
                :key="i"
                type="button"
                class="relative size-12 shrink-0 cursor-pointer rounded-xl transition"
                :class="i === bg ? 'ring-2 ring-primary-400 ring-offset-2 ring-offset-[var(--ui-bg)]' : 'hover:scale-105'"
                :style="{ background: css(g) }"
                :aria-label="`${$t('iconEditor.background')} ${i + 1}`"
                @click="bg = i"
              >
                <UIcon v-if="i === bg" name="i-lucide-check" class="absolute top-1 right-1 size-3.5 text-white drop-shadow" />
              </button>
            </div>
          </section>

          <section>
            <p class="mb-2 text-sm font-medium">{{ $t('iconEditor.symbol') }}</p>
            <div class="grid max-h-64 grid-cols-6 gap-2 overflow-y-auto pr-1">
              <button
                type="button"
                class="flex aspect-square cursor-pointer items-center justify-center rounded-xl border border-dashed border-default text-muted transition hover:border-primary-400 hover:text-primary-400"
                :title="$t('iconEditor.addSymbol')"
                :aria-label="$t('iconEditor.addSymbol')"
                :disabled="adding"
                @click="addSymbol"
              >
                <UIcon :name="adding ? 'i-lucide-loader-circle' : 'i-lucide-plus'" class="size-5" :class="adding && 'animate-spin'" />
              </button>
              <div v-for="s in symbols" :key="s.id" class="group/sym relative">
                <button
                  type="button"
                  class="flex aspect-square w-full cursor-pointer items-center justify-center rounded-xl border p-1.5 transition"
                  :class="s.id === symbol?.id ? 'border-primary-400 bg-primary-500/10' : 'border-default bg-white/3 hover:bg-white/6'"
                  :aria-label="s.id"
                  @click="symbol = s"
                >
                  <img :src="s.url" :alt="s.id" class="size-full object-contain" loading="lazy" />
                </button>
                <!-- only the user's own can go; the bundled ones are read-only -->
                <UButton
                  v-if="s.path"
                  icon="i-lucide-x"
                  color="error"
                  variant="solid"
                  size="xs"
                  class="absolute -top-1.5 -right-1.5 rounded-full opacity-0 transition group-hover/sym:opacity-100"
                  :title="$t('common.remove')"
                  @click="removeSymbol(s)"
                />
              </div>
            </div>
          </section>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full items-center gap-3">
        <ModalHint>{{ $t('iconEditor.hint') }}</ModalHint>
        <div class="ml-auto flex shrink-0 gap-2">
          <UButton icon="i-lucide-x" variant="ghost" color="neutral" :label="$t('common.cancel')" @click="open = false" />
          <UButton icon="i-lucide-save" :loading="saving" :label="$t('iconEditor.save')" @click="save" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'

// Without an `instanceId` (creating an instance, which has no id yet) the
// composed PNG is only handed back, for the caller to apply once it has one.
const props = defineProps<{ instanceId?: string }>()
const emit = defineEmits<{ (e: 'saved', dataUrl: string): void }>()
const open = defineModel<boolean>('open', { default: false })

const toast = useToast()
const { t } = useI18n()

const SYMBOL_DIR = '/instance-symbols'

/** `path` is set for the user's own symbols, which live in the data folder. */
interface SymbolItem { id: string, url: string, path?: string }

// The folder is the source of truth: globbing it at build time means dropping a
// new PNG in there is all it takes to add a symbol. Only the names are used —
// the files are served from `public/`, not bundled a second time.
const BUILT_IN: SymbolItem[] = Object.keys(import.meta.glob('../../public/instance-symbols/*.png'))
  .map(path => path.split('/').pop()!)
  .sort()
  .map(id => ({ id, url: `${SYMBOL_DIR}/${id}` }))

const custom = ref<SymbolItem[]>([])
// The user's own come first — they are the ones being looked for.
const symbols = computed<SymbolItem[]>(() => [...custom.value, ...BUILT_IN])

async function loadCustom() {
  try {
    const paths = await invoke<string[]>('list_custom_symbols')
    custom.value = paths.map(path => ({ id: path, url: convertFileSrc(path), path }))
  } catch {
    custom.value = []
  }
}

/** Two stops each, drawn top-left → bottom-right in both CSS and the canvas. */
const GRADIENTS: [string, string][] = [
  ['#f43f5e', '#e11d48'],
  ['#fb7185', '#f97316'],
  ['#fb923c', '#f59e0b'],
  ['#fbbf24', '#eab308'],
  ['#a3e635', '#65a30d'],
  ['#4ade80', '#16a34a'],
  ['#34d399', '#0d9488'],
  ['#22d3ee', '#0284c7'],
  ['#60a5fa', '#4f46e5'],
  ['#818cf8', '#7c3aed'],
  ['#c084fc', '#9333ea'],
  ['#f472b6', '#db2777'],
  ['#94a3b8', '#475569'],
  ['#334155', '#0f172a'],
]

const bg = ref(0)
const symbol = ref<SymbolItem | null>(BUILT_IN[0] ?? null)

const css = (g: [string, string]) => `linear-gradient(135deg, ${g[0]}, ${g[1]})`

/** Share of the tile the symbol covers; the canvas uses the same number. */
const SYMBOL_SCALE = 0.72
const PAD = `${((1 - SYMBOL_SCALE) / 2) * 100}%`

const tileStyle = computed(() => ({ background: css(GRADIENTS[bg.value]!) }))
const symbolStyle = { padding: PAD }
const symbolUrl = computed(() => symbol.value?.url ?? '')

const pick = <T,>(list: readonly T[]) => list[Math.floor(Math.random() * list.length)]!

function randomize() {
  bg.value = Math.floor(Math.random() * GRADIENTS.length)
  symbol.value = pick(symbols.value)
}

// Open on something other than the first swatch every time.
watch(open, async (isOpen) => {
  if (!isOpen) return
  await loadCustom()
  randomize()
}, { immediate: true })

// --- the user's own symbols ---
const adding = ref(false)

async function addSymbol() {
  adding.value = true
  try {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    })
    if (typeof picked !== 'string') return
    const path = await invoke<string>('add_custom_symbol', { sourcePath: picked })
    await loadCustom()
    symbol.value = custom.value.find(s => s.path === path) ?? symbol.value
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    adding.value = false
  }
}

async function removeSymbol(s: SymbolItem) {
  try {
    await invoke('delete_custom_symbol', { path: s.path })
    await loadCustom()
    if (symbol.value?.id === s.id) symbol.value = symbols.value[0] ?? null
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error(`load ${src}`))
    img.src = src
  })
}

/** Draws what the preview shows into a 512² PNG data URL. */
async function compose(): Promise<string> {
  const size = 512
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = size
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas unavailable')

  const [from, to] = GRADIENTS[bg.value]!
  const gradient = ctx.createLinearGradient(0, 0, size, size)
  gradient.addColorStop(0, from)
  gradient.addColorStop(1, to)
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, size, size)

  // Drawing an asset-protocol image would taint the canvas and make toDataURL
  // throw, so the user's own symbols are read as a data: URL first.
  const src = symbol.value?.path
    ? await invoke<string>('read_image_data_url', { path: symbol.value.path })
    : symbolUrl.value
  const img = await loadImage(src)
  const drawn = size * SYMBOL_SCALE
  const offset = (size - drawn) / 2
  ctx.drawImage(img, offset, offset, drawn, drawn)

  return canvas.toDataURL('image/png')
}

const saving = ref(false)

async function save() {
  saving.value = true
  try {
    const dataUrl = await compose()
    if (props.instanceId) {
      await invoke('set_instance_icon_data', { id: props.instanceId, dataUrl })
      invalidateInstanceIcon(props.instanceId)
      toast.add({ title: t('instance.iconChanged'), color: 'success' })
    }
    emit('saved', dataUrl)
    open.value = false
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    saving.value = false
  }
}
</script>
