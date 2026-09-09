<template>
  <div class="flex h-full flex-col">
    <div class="min-h-0 flex-1 overflow-y-auto p-6 lg:p-8">
      <div class="mx-auto grid max-w-6xl grid-cols-1 gap-8 lg:grid-cols-[360px_1fr]">
        <div class="lg:sticky lg:top-0 lg:self-start">
          <h1 class="mb-4 text-2xl font-bold tracking-tight">{{ $t('skins.title') }}</h1>

          <div class="flex flex-col items-center gap-4 rounded-2xl border border-default bg-linear-[160deg] from-primary-500/10 to-transparent p-5">
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

            <div class="flex w-full flex-wrap items-center justify-center gap-2">
              <UButton
                v-if="canApply"
                icon="i-lucide-check"
                :loading="applying"
                :disabled="!isMicrosoft"
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

              <UButton
                icon="i-lucide-shirt"
                color="neutral"
                variant="soft"
                :label="$t('skins.capesButton')"
                @click="openCapes"
              />
            </div>

            <div v-if="activeCape" class="flex items-center gap-2 text-xs text-muted">
              <span class="w-4 h-6 rounded-sm ring-1 ring-white/10" :style="capeStyle(activeCape.url)" />
              {{ activeCape.alias }}
            </div>
          </div>
        </div>

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
                  <div class="h-20 animate-pulse rounded-lg bg-white/5" />
                </div>
              </template>

              <div
                v-for="s in saved"
                :key="s.id"
                class="group relative flex w-24 cursor-pointer flex-col items-center gap-1 rounded-xl border p-2 transition"
                :class="selected.kind === 'saved' && selected.id === s.id ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
                @click="previewSaved(s)"
              >
                <img v-if="savedBust[s.id]" :src="savedBust[s.id]" class="h-20 w-full object-contain" :alt="s.name" >
                <div v-else class="h-20 w-full animate-pulse rounded-lg bg-white/5" />
                <span class="w-full truncate text-center text-[11px] text-neutral-300" :title="s.name">{{ s.name }}</span>
                <UIcon
                  v-if="s.active"
                  name="i-lucide-circle-check"
                  class="absolute top-1.5 left-1.5 size-4 text-success"
                  :title="$t('skins.inUse')"
                />
                <button
                  type="button"
                  class="absolute top-1 right-1 hidden rounded-md bg-black/60 p-1 text-neutral-300 transition group-hover:block hover:text-error"
                  :title="$t('common.remove')"
                  @click.stop="remove(s)"
                >
                  <UIcon name="i-lucide-trash-2" class="size-3.5" />
                </button>
              </div>

              <p v-if="!savedLoading && !saved.length" class="self-center text-sm text-muted">{{ $t('skins.noSaved') }}</p>
            </div>
          </section>

          <section>
            <h2 class="mb-3 text-sm font-semibold text-neutral-300">{{ $t('skins.defaults') }}</h2>
            <p v-if="!defaults.length" class="text-sm text-muted">{{ $t('skins.defaultsEmpty') }}</p>
            <div class="flex flex-wrap gap-3">
              <button
                v-for="d in defaults"
                :key="d.name"
                type="button"
                class="flex w-24 flex-col items-center gap-1 rounded-xl border p-2 transition"
                :class="selected.kind === 'default' && selected.name === d.name ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
                @click="previewDefault(d)"
              >
                <img v-if="defaultBust[d.name]" :src="defaultBust[d.name]" class="h-20 w-full object-contain" :alt="d.name" >
                <div v-else class="h-20 w-full animate-pulse rounded-lg bg-white/5" />
                <span class="w-full truncate text-center text-[11px] text-neutral-300">{{ d.name }}</span>
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>

    <div v-if="!isMicrosoft" class="shrink-0 border-t border-default bg-white/2 px-6 py-4 lg:px-8">
      <div class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-3">
        <div class="flex min-w-0 items-start gap-3">
          <UIcon name="i-lucide-info" class="mt-0.5 size-5 shrink-0 text-primary-400" />
          <div class="min-w-0">
            <p class="text-sm font-semibold">{{ $t('skins.demoTitle') }}</p>
            <p class="text-sm text-muted">{{ $t('skins.demoDesc') }}</p>
          </div>
        </div>
        <UButton
          icon="i-simple-icons-microsoft"
          :loading="signingIn"
          :label="$t('skins.signIn')"
          @click="signIn"
        />
      </div>
    </div>

    <UModal v-model:open="capesOpen" :title="$t('skins.capes')" :ui="{ content: 'max-w-2xl' }">
      <template #body>
        <p v-if="!isMicrosoft" class="mb-4 text-sm text-muted">{{ $t('skins.capesDemo') }}</p>

        <div v-if="capesLoading" class="flex flex-wrap gap-3">
          <div v-for="n in 3" :key="`cape-sk-${n}`" class="h-28 w-24 animate-pulse rounded-xl bg-white/5" />
        </div>

        <div v-else class="flex flex-wrap gap-3">
          <button
            type="button"
            class="flex w-24 flex-col items-center gap-2 rounded-xl border p-2 transition"
            :class="!activeCapeId ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
            :disabled="!isMicrosoft"
            @click="chooseCape(null)"
          >
            <div class="flex h-32 w-full items-center justify-center rounded-lg bg-white/5">
              <UIcon name="i-lucide-slash" class="size-5 text-neutral-500" />
            </div>
            <span class="text-[11px] text-neutral-300">{{ $t('skins.noCape') }}</span>
          </button>

          <button
            v-for="c in capes"
            :key="c.id"
            type="button"
            class="flex w-24 flex-col items-center gap-2 rounded-xl border p-2 transition"
            :class="c.active ? 'border-primary-500 bg-primary-500/10' : 'border-default bg-white/3 hover:border-primary-500/40'"
            :disabled="!isMicrosoft"
            @click="chooseCape(c)"
          >
            <span class="h-32 w-full rounded-lg" :style="capeStyle(c.url)" />
            <span class="w-full truncate text-center text-[11px] text-neutral-300" :title="c.alias">{{ c.alias }}</span>
          </button>

          <p v-if="!capes.length" class="self-center text-sm text-muted">{{ $t('skins.capesEmpty') }}</p>
        </div>
      </template>
    </UModal>
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
const nickname = computed(() => account.activeAccount?.username ?? 'Player')

interface Cape { id: string, url: string, alias: string, active: boolean }
interface DefaultSkin { name: string, model: 'classic' | 'slim', path: string | null, url: string | null }

const capes = ref<Cape[]>([])
const capesLoading = ref(false)
const capesOpen = ref(false)
const activeCapeId = computed(() => capes.value.find(c => c.active)?.id ?? null)
const activeCape = computed(() => capes.value.find(c => c.active) ?? null)

const defaults = ref<DefaultSkin[]>([])
const defaultRaw = reactive<Record<string, string>>({})
const defaultBust = reactive<Record<string, string>>({})

const bust = useSkinBust()
const saved = ref<SavedSkin[]>([])
const savedLoading = ref(true)
const savedRaw = reactive<Record<string, string>>({})
const savedBust = reactive<Record<string, string>>({})
const selected = ref<{ kind: 'player' | 'saved' | 'default', id?: string, name?: string }>({ kind: 'player' })
const applying = ref(false)
const signingIn = ref(false)
const dragOver = ref(false)

const selectedSavedSkin = computed(() =>
  selected.value.kind === 'saved' ? saved.value.find(s => s.id === selected.value.id) : undefined,
)
const selectedSavedActive = computed(() => !!selectedSavedSkin.value?.active)
const canApply = computed(() => selected.value.kind === 'saved' && !selectedSavedActive.value)

const viewerCanvas = ref<HTMLCanvasElement>()
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let viewer: any = null

const svModel = (m: 'classic' | 'slim') => (m === 'slim' ? 'slim' : 'default')

async function loadIntoViewer(skin: string, model: 'classic' | 'slim') {
  if (!viewer) return
  await viewer.loadSkin(skin, { model: svModel(model) })
}

async function skinDataUrl(d: DefaultSkin): Promise<string> {
  if (defaultRaw[d.name]) return defaultRaw[d.name]!
  const data = d.path
    ? await invoke<string>('read_image_data_url', { path: d.path })
    : await invoke<string>('fetch_skin_data_url', { url: d.url })
  defaultRaw[d.name] = data
  return data
}

async function loadCapes() {
  if (!isMicrosoft.value) {
    capes.value = []
    return
  }
  capesLoading.value = true
  try {
    capes.value = await invoke<Cape[]>('get_player_capes')
  } catch {
    capes.value = []
  } finally {
    capesLoading.value = false
  }
  applyCapeToViewer()
}

function applyCapeToViewer() {
  if (!viewer) return
  const cape = activeCape.value
  if (cape) viewer.loadCape(cape.url).catch(() => {})
  else viewer.resetCape?.()
}

async function chooseCape(c: Cape | null) {
  if (!isMicrosoft.value) return
  try {
    await invoke('set_active_cape', { capeId: c?.id ?? null })
    capes.value = capes.value.map(x => ({ ...x, active: x.id === c?.id }))
    applyCapeToViewer()
    toast.add({ title: t('skins.capeSet'), color: 'success' })
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

function openCapes() {
  capesOpen.value = true
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
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function showPlayer() {
  selected.value = { kind: 'player' }
  const fallback = defaults.value[0]
  if (!isMicrosoft.value || !account.activeAccount) {
    if (fallback) await loadIntoViewer(await skinDataUrl(fallback), fallback.model)
    return
  }
  try {
    const ps = await invoke<PlayerSkin>('get_player_skin', { uuid: account.activeAccount.uuid })
    await loadIntoViewer(ps.skin, ps.slim ? 'slim' : 'classic')
  } catch {
    if (fallback) await loadIntoViewer(await skinDataUrl(fallback), fallback.model)
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
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function previewDefault(d: DefaultSkin) {
  selected.value = { kind: 'default', name: d.name }
  try {
    await loadIntoViewer(await skinDataUrl(d), d.model)
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  }
}

async function loadDefaults() {
  try {
    defaults.value = await invoke<DefaultSkin[]>('list_default_skins')
  } catch {
    defaults.value = []
  }
  for (const d of defaults.value) {
    if (defaultBust[d.name]) continue
    try {
      defaultBust[d.name] = await bust.render(await skinDataUrl(d), d.model)
    } catch {
      defaultBust[d.name] = ''
    }
  }
}

async function applySelected() {
  if (!isMicrosoft.value) {
    toast.add({ title: t('skins.loginHint'), color: 'error' })
    return
  }
  if (selected.value.kind !== 'saved' || !selected.value.id) return
  applying.value = true
  try {
    await invoke('apply_skin', { id: selected.value.id })
    toast.add({ title: t('skins.applied'), color: 'success' })
    await loadSaved()
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    applying.value = false
  }
}

async function signIn() {
  signingIn.value = true
  try {
    await account.login()
    await loadCapes()
    await initPlayerSkin()
  } catch (e) {
    toast.add({ title: errorText(e), color: 'error' })
  } finally {
    signingIn.value = false
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
    } catch {
      savedBust[s.id] = ''
    }
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
    toast.add({ title: errorText(e), color: 'error' })
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
    toast.add({ title: errorText(e), color: 'error' })
  }
}

let unlistenDrop: UnlistenFn | null = null

onMounted(async () => {
  if (!account.accounts.length) await account.load()

  const { SkinViewer, IdleAnimation } = await import('skinview3d')
  viewer = new SkinViewer({ canvas: viewerCanvas.value!, width: 300, height: 400 })
  viewer.animation = new IdleAnimation()
  viewer.animation.speed = 0.7
  viewer.zoom = 0.85
  viewer.autoRotate = true
  viewer.autoRotateSpeed = 0.45
  viewer.controls.enablePan = false
  viewer.controls.enableZoom = false
  viewer.nameTag = nickname.value

  viewerCanvas.value?.addEventListener('pointerdown', pauseRotation)
  viewerCanvas.value?.addEventListener('pointerup', resumeRotation)
  viewerCanvas.value?.addEventListener('pointerleave', resumeRotation)

  await loadDefaults()
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

function pauseRotation() {
  if (viewer) viewer.autoRotate = false
}

function resumeRotation() {
  if (viewer) viewer.autoRotate = true
}

watch(nickname, (name) => {
  if (viewer) viewer.nameTag = name
})

onBeforeUnmount(() => {
  unlistenDrop?.()
  viewerCanvas.value?.removeEventListener('pointerdown', pauseRotation)
  viewerCanvas.value?.removeEventListener('pointerup', resumeRotation)
  viewerCanvas.value?.removeEventListener('pointerleave', resumeRotation)
  viewer?.dispose?.()
})
</script>
