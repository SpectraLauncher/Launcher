<template>
  <div class="h-full overflow-y-auto p-6 lg:p-8">
    <div class="mx-auto max-w-6xl space-y-8">
      <div class="grid grid-cols-1 gap-6 lg:grid-cols-[320px_1fr]">
        <section class="flex flex-col items-center gap-3 rounded-2xl border border-default bg-linear-[160deg] from-primary-500/10 to-transparent p-5">
          <div class="text-lg font-bold tracking-tight">{{ nickname }}</div>
          <canvas ref="viewerCanvas" class="rounded-xl" />

          <div v-if="selectedSavedSkin" class="flex w-full max-w-52 rounded-lg border border-default p-0.5 text-xs">
            <button
              v-for="m in (['classic', 'slim'] as const)"
              :key="m"
              type="button"
              class="flex-1 rounded-md py-1 font-medium transition"
              :class="selectedSavedSkin.model === m ? 'bg-primary-500/20 text-primary-300' : 'text-neutral-400 hover:text-neutral-200'"
              @click="setModel(m)"
            >
              {{ $t(`skins.${m}`) }}
            </button>
          </div>

          <UButton
            v-if="canApply"
            icon="i-lucide-upload"
            :loading="applying"
            :label="$t('skins.apply')"
            @click="applySelected"
          />
          <UBadge
            v-else-if="selectedSavedActive"
            color="success"
            variant="subtle"
            icon="i-lucide-circle-check"
            :label="$t('skins.inUse')"
          />
          <p v-if="!isMicrosoft" class="text-center text-xs text-muted">{{ $t('skins.loginHint') }}</p>
        </section>

        <div class="space-y-8">
          <section>
            <h2 class="mb-3 text-sm font-semibold text-neutral-300">{{ $t('skins.saved') }}</h2>
            <div class="flex flex-wrap gap-3">
              <button
                type="button"
                class="flex h-28 w-24 flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed p-2 text-center transition"
                :class="dragOver ? 'border-primary-500 bg-primary-500/10' : 'border-default text-neutral-400 hover:border-primary-500/60 hover:text-neutral-200'"
                @click="pickFile"
              >
                <UIcon name="i-lucide-plus" class="size-6" />
                <span class="text-[11px] leading-tight">{{ $t('skins.addHint') }}</span>
              </button>

              <template v-if="savedLoading">
                <div v-for="n in 4" :key="`sk-${n}`" class="w-24 rounded-xl border border-default bg-white/3 p-2">
                  <div class="mx-auto h-22 w-full animate-pulse rounded bg-white/5" />
                  <div class="mx-auto mt-1.5 h-3 w-3/4 animate-pulse rounded bg-white/5" />
                </div>
              </template>

              <template v-else>
                <div
                  v-for="s in saved"
                  :key="s.id"
                  class="group relative w-24 cursor-pointer rounded-xl border p-2 transition"
                  :class="selected.id === s.id ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
                  @click="previewSaved(s)"
                >
                  <UBadge
                    v-if="s.active"
                    color="success"
                    variant="solid"
                    size="xs"
                    :label="$t('skins.inUse')"
                    class="absolute top-1 left-1 z-1"
                  />
                  <img v-if="savedBust[s.id]" :src="savedBust[s.id]" class="mx-auto h-22 w-full object-contain" :alt="s.name" >
                  <div v-else class="mx-auto h-22 w-full animate-pulse rounded bg-white/5" />
                  <div class="mt-1.5 truncate text-center text-[11px]" :title="s.name">{{ s.name }}</div>
                  <UButton
                    icon="i-lucide-trash-2"
                    color="error"
                    variant="solid"
                    size="xs"
                    class="absolute -top-1.5 -right-1.5 opacity-0 transition group-hover:opacity-100"
                    :title="$t('common.remove')"
                    @click.stop="remove(s)"
                  />
                </div>

                <p v-if="!saved.length" class="self-center text-sm text-muted">{{ $t('skins.noSaved') }}</p>
              </template>
            </div>
          </section>

          <section>
            <h2 class="mb-3 text-sm font-semibold text-neutral-300">{{ $t('skins.defaults') }}</h2>
            <div class="grid gap-3" style="grid-template-columns:repeat(auto-fill,minmax(96px,1fr))">
              <div
                v-for="d in defaults"
                :key="d.name"
                class="group cursor-pointer rounded-xl border p-2 transition"
                :class="selected.kind === 'default' && selected.name === d.name ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
                @click="previewDefault(d)"
              >
                <img v-if="defaultBust[d.name]" :src="defaultBust[d.name]" class="mx-auto h-22 w-full object-contain" :alt="d.name" >
                <div v-else class="mx-auto h-22 w-full animate-pulse rounded bg-white/5" />
                <div class="mt-1.5 truncate text-center text-[11px]">{{ d.name }}</div>
              </div>
            </div>
          </section>
        </div>
      </div>

      <section v-if="isMicrosoft && (capesLoading || capes.length)">
        <h2 class="mb-3 text-sm font-semibold text-neutral-300">{{ $t('skins.capes') }}</h2>
        <div class="flex flex-wrap gap-3">
          <template v-if="capesLoading">
            <div v-for="n in 3" :key="`cape-sk-${n}`" class="flex w-24 flex-col items-center gap-2 rounded-xl border border-default bg-white/3 p-2">
              <div class="h-16 w-10 animate-pulse rounded bg-white/5" />
              <div class="h-3 w-3/5 animate-pulse rounded bg-white/5" />
            </div>
          </template>

          <template v-else>
            <button
              type="button"
              class="flex w-24 flex-col items-center gap-2 rounded-xl border p-2 transition"
              :class="!activeCapeId ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
              @click="chooseCape(null)"
            >
              <div class="flex h-16 w-10 items-center justify-center rounded bg-white/5 text-neutral-500">
                <UIcon name="i-lucide-ban" class="size-5" />
              </div>
              <span class="text-[11px]">{{ $t('skins.noCape') }}</span>
            </button>
            <button
              v-for="c in capes"
              :key="c.id"
              type="button"
              class="flex w-24 flex-col items-center gap-2 rounded-xl border p-2 transition"
              :class="c.active ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
              @click="chooseCape(c)"
            >
              <div class="h-16 w-10 rounded" :style="capeStyle(c.url)" />
              <span class="truncate text-[11px]" :title="c.alias">{{ c.alias || 'Cape' }}</span>
            </button>
          </template>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { SavedSkin, PlayerSkin } from '~/types/launcher'

const account = useAccountStore()
const toast = useToast()
const { t } = useI18n()

const isMicrosoft = computed(() => account.activeAccount?.kind === 'microsoft')
const nickname = computed(() => account.activeAccount?.username ?? '—')

interface Cape { id: string; url: string; alias: string; active: boolean }
const capes = ref<Cape[]>([])
const capesLoading = ref(false)
const activeCapeId = computed(() => capes.value.find(c => c.active)?.id ?? null)

async function loadCapes() {
  if (!isMicrosoft.value) { capes.value = []; return }
  capesLoading.value = true
  try { capes.value = await invoke<Cape[]>('get_player_capes') }
  catch { capes.value = [] }
  finally { capesLoading.value = false }
}
async function chooseCape(c: Cape | null) {
  try {
    await invoke('set_active_cape', { capeId: c?.id ?? null })
    capes.value = capes.value.map(x => ({ ...x, active: x.id === c?.id }))
    toast.add({ title: t('skins.capeSet'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}
function capeStyle(url: string) {
  return {
    backgroundImage: `url(${url})`,
    backgroundSize: '640% 200%',
    backgroundPosition: '1.85% 6.25%',
    backgroundRepeat: 'no-repeat',
    imageRendering: 'pixelated' as const,
  }
}
watch(isMicrosoft, loadCapes)

interface DefaultSkin { name: string; model: 'classic' | 'slim'; url: string }
const defaults: DefaultSkin[] = [
  { name: 'Steve', model: 'classic', url: 'https://assets.mojang.com/SkinTemplates/steve.png' },
  { name: 'Alex', model: 'slim', url: 'https://assets.mojang.com/SkinTemplates/alex.png' },
]

const bust = useSkinBust()
const saved = ref<SavedSkin[]>([])
const savedLoading = ref(true)
const savedRaw = reactive<Record<string, string>>({})
const savedBust = reactive<Record<string, string>>({})
const defaultRaw = reactive<Record<string, string>>({})
const defaultBust = reactive<Record<string, string>>({})
const selected = ref<{ kind: 'player' | 'saved' | 'default'; id?: string; name?: string }>({ kind: 'player' })
const applying = ref(false)
const dragOver = ref(false)

const selectedSavedSkin = computed(() =>
  selected.value.kind === 'saved' ? saved.value.find(s => s.id === selected.value.id) : undefined,
)
const selectedSavedActive = computed(() => !!selectedSavedSkin.value?.active)
const canApply = computed(() => isMicrosoft.value && selected.value.kind === 'saved' && !selectedSavedActive.value)

async function setModel(model: 'classic' | 'slim') {
  const s = selectedSavedSkin.value
  if (!s || s.model === model) return
  s.model = model
  try {
    await invoke('set_skin_model', { id: s.id, model })
    const data = savedRaw[s.id] ?? (await invoke<string>('get_skin_data_url', { id: s.id }))
    savedRaw[s.id] = data
    savedBust[s.id] = await bust.render(data, model)
    await loadIntoViewer(data, model)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

const viewerCanvas = ref<HTMLCanvasElement>()
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let viewer: any = null

const svModel = (m: 'classic' | 'slim') => (m === 'slim' ? 'slim' : 'default')

async function loadIntoViewer(skin: string, model: 'classic' | 'slim') {
  if (!viewer) return
  await viewer.loadSkin(skin, { model: svModel(model) })
}

async function showPlayer() {
  selected.value = { kind: 'player' }
  if (!isMicrosoft.value || !account.activeAccount) {
    await loadIntoViewer(defaults[0]!.url, 'classic')
    return
  }
  try {
    const ps = await invoke<PlayerSkin>('get_player_skin', { uuid: account.activeAccount.uuid })
    await loadIntoViewer(ps.skin, ps.slim ? 'slim' : 'classic')
  } catch {
    await loadIntoViewer(defaults[0]!.url, 'classic')
  }
}

async function initPlayerSkin() {
  if (!isMicrosoft.value || !account.activeAccount) {
    await showPlayer()
    return
  }
  try {
    const s = await invoke<SavedSkin>('import_player_skin', {
      uuid: account.activeAccount.uuid,
      name: nickname.value,
    })
    await loadSaved()
    const imported = saved.value.find(x => x.id === s.id)
    if (imported) await previewSaved(imported)
    else await showPlayer()
  } catch {
    await showPlayer()
  }
}

async function previewSaved(s: SavedSkin) {
  selected.value = { kind: 'saved', id: s.id, name: s.name }
  try {
    const data = savedRaw[s.id] ?? (await invoke<string>('get_skin_data_url', { id: s.id }))
    savedRaw[s.id] = data
    await loadIntoViewer(data, s.model)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function loadDefaultBusts() {
  for (const d of defaults) {
    if (defaultBust[d.name]) continue
    try {
      const data = await invoke<string>('fetch_skin_data_url', { url: d.url })
      defaultRaw[d.name] = data
      defaultBust[d.name] = await bust.render(data, d.model)
    } catch {  }
  }
}

async function previewDefault(d: DefaultSkin) {
  selected.value = { kind: 'default', name: d.name }
  try {
    const data = defaultRaw[d.name] ?? (await invoke<string>('fetch_skin_data_url', { url: d.url }))
    await loadIntoViewer(data, d.model)
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function applySelected() {
  if (selected.value.kind !== 'saved' || !selected.value.id) return
  applying.value = true
  try {
    await invoke('apply_skin', { id: selected.value.id })
    toast.add({ title: t('skins.applied'), color: 'success' })
    await loadSaved()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  } finally {
    applying.value = false
  }
}

async function loadSaved() {
  try {
    saved.value = await invoke<SavedSkin[]>('list_skins')
  } finally {
    savedLoading.value = false
  }
  for (const s of saved.value) {
    if (savedBust[s.id]) continue
    try {
      const data = await invoke<string>('get_skin_data_url', { id: s.id })
      savedRaw[s.id] = data
      savedBust[s.id] = await bust.render(data, s.model)
    } catch {  }
  }
}

function baseName(path: string): string {
  const file = path.replace(/\\/g, '/').split('/').pop() ?? 'skin.png'
  return file.replace(/\.png$/i, '')
}

async function addFromPath(path: string) {
  if (!path.toLowerCase().endsWith('.png')) return
  try {
    await invoke<SavedSkin>('save_skin', { name: baseName(path), model: 'classic', sourcePath: path })
    await loadSaved()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function pickFile() {
  const selectedPath = await open({ multiple: false, directory: false, filters: [{ name: 'Skin', extensions: ['png'] }] })
  if (typeof selectedPath === 'string') await addFromPath(selectedPath)
}

async function remove(s: SavedSkin) {
  try {
    await invoke('delete_skin', { id: s.id })
    delete savedRaw[s.id]
    delete savedBust[s.id]
    if (selected.value.id === s.id) await showPlayer()
    await loadSaved()
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

let unlistenDrop: UnlistenFn | null = null

onMounted(async () => {
  if (!account.accounts.length) await account.load()
  loadDefaultBusts()

  const { SkinViewer, IdleAnimation } = await import('skinview3d')
  viewer = new SkinViewer({ canvas: viewerCanvas.value!, width: 280, height: 380 })
  viewer.animation = new IdleAnimation()
  viewer.zoom = 0.9
  viewer.autoRotate = false

  await loadSaved()
  await initPlayerSkin()
  await loadCapes()

  unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload
    if (p.type === 'over' || p.type === 'enter') dragOver.value = true
    else if (p.type === 'leave') dragOver.value = false
    else if (p.type === 'drop') {
      dragOver.value = false
      for (const path of p.paths) addFromPath(path)
    }
  })
})

onBeforeUnmount(() => {
  unlistenDrop?.()
  viewer?.dispose?.()
})
</script>
