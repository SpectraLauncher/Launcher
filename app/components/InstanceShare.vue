<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { SharePreview, ShareResult, UnresolvedFile } from '~/types/launcher'
import type { SpectraFriend } from '~/composables/useSpectraAccount'

const props = defineProps<{ instanceId: string, instanceName: string }>()

const { t } = useI18n()
const account = useSpectraAccount()
const activity = useActivityCenter()

interface Recipient {
  user: { id: string, name: string | null, username: string | null, image: string | null }
  importedRevision: number | null
  outdated: boolean
}
interface OwnedShare {
  code: string
  instance_id: string | null
  name: string | null
  revision: number
  downloads: number
  expires: number
  canExtend: boolean
  recipients: Recipient[]
}

const preview = ref<SharePreview | null>(null)
const loadingPreview = ref(true)

const progress = ref<{ stage: string, current: number, total: number } | null>(null)

const STAGE_WEIGHTS: Record<string, number> = {
  scanning: 0.05,
  packing: 0.35,
  uploading: 0.55,
  finishing: 0.05,
}
const STAGE_ORDER = ['scanning', 'packing', 'uploading', 'finishing']

const stageFraction = computed(() => {
  const p = progress.value
  if (!p || !p.total) return null
  return Math.min(1, p.current / p.total)
})

const stagePercent = computed(() =>
  stageFraction.value === null ? null : Math.round(stageFraction.value * 100))

const overallPercent = computed(() => {
  const p = progress.value
  if (!p) return 0
  if (p.stage === 'done') return 100
  const index = STAGE_ORDER.indexOf(p.stage)
  if (index < 0) return 0
  const before = STAGE_ORDER.slice(0, index).reduce((n, s) => n + (STAGE_WEIGHTS[s] ?? 0), 0)
  const inside = (STAGE_WEIGHTS[p.stage] ?? 0) * (stageFraction.value ?? 0)
  return Math.round((before + inside) * 100)
})

const selected = ref(new Set<string>())
const expanded = ref(new Set<string>())

const groups = computed(() => {
  const map = new Map<string, UnresolvedFile[]>()
  for (const file of preview.value?.unresolved ?? []) {
    const dir = file.path.split('/')[0] ?? ''
    map.set(dir, [...(map.get(dir) ?? []), file])
  }
  return [...map.entries()]
    .map(([dir, files]) => ({
      dir,
      files,
      bytes: files.reduce((n, f) => n + f.size, 0),
    }))
    .sort((a, b) => a.dir.localeCompare(b.dir))
})

const selectedBytes = computed(() =>
  (preview.value?.unresolved ?? [])
    .filter(f => selected.value.has(f.path))
    .reduce((n, f) => n + f.size, 0))

function groupState(files: UnresolvedFile[]) {
  const on = files.filter(f => selected.value.has(f.path)).length
  if (on === 0) return 'unchecked'
  return on === files.length ? 'checked' : 'indeterminate'
}

function toggleFile(path: string) {
  const next = new Set(selected.value)
  next.has(path) ? next.delete(path) : next.add(path)
  selected.value = next
}

function toggleGroup(files: UnresolvedFile[]) {
  const next = new Set(selected.value)
  const turnOn = groupState(files) !== 'checked'
  for (const f of files) turnOn ? next.add(f.path) : next.delete(f.path)
  selected.value = next
}

function toggleExpand(dir: string) {
  const next = new Set(expanded.value)
  next.has(dir) ? next.delete(dir) : next.add(dir)
  expanded.value = next
}

const boxClass = (state: string) =>
  state === 'unchecked' ? 'border-white/25 hover:border-white/40' : 'border-primary-500 bg-primary-500'

function fmtSize(n: number) {
  if (n < 1024) return `${n} B`
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(0)} KB`
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toFixed(1)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}
const share = ref<OwnedShare | null>(null)
const anonymous = ref<ShareResult | null>(null)
const friends = ref<SpectraFriend[]>([])
const busy = ref('')
const error = ref('')
const copied = ref('')

const shareUrl = computed(() =>
  share.value ? `https://spectra.makoto.com.pl/s/${share.value.code}` : anonymous.value?.url ?? '')
const code = computed(() => share.value?.code ?? anonymous.value?.code ?? '')

const invitable = computed(() => {
  const have = new Set(share.value?.recipients.map(r => r.user.id) ?? [])
  return friends.value.filter(f => !have.has(f.id))
})

async function run(key: string, fn: () => Promise<unknown>) {
  busy.value = key
  error.value = ''
  try {
    await fn()
  } catch (e) {
    error.value = String(e)
  } finally {
    busy.value = ''
  }
}

async function loadPreview() {
  loadingPreview.value = true
  try {
    preview.value = await invoke<SharePreview>('share_preview', { id: props.instanceId })
  } catch (e) {
    error.value = String(e)
  } finally {
    loadingPreview.value = false
  }
}

async function loadShare() {
  if (!account.isSignedIn.value) return
  try {
    const res = await account.api<{ shares: OwnedShare[] }>('GET', '/api/shares')
    share.value = res.shares.find(s => s.instance_id === props.instanceId) ?? null
  } catch (e) {
    error.value = String(e)
  }
}

async function loadFriends() {
  if (!account.isSignedIn.value) return
  try {
    const res = await account.api<{ friends: SpectraFriend[] }>('GET', '/api/friends')
    friends.value = res.friends
  } catch {  }
}

const upload = () => run('upload', async () => {
  progress.value = { stage: 'scanning', current: 0, total: 0 }
  try {
    const result = await activity.withTask(t('share.working', { name: props.instanceName }), () =>
      invoke<ShareResult>('share_instance', {
        id: props.instanceId,
        include: [...selected.value],
      }))
    if (account.isSignedIn.value) await loadShare()
    else anonymous.value = result
  } finally {
    progress.value = null
  }
})

const confirmingRevoke = ref(false)
let revokeTimer: ReturnType<typeof setTimeout> | null = null

function askRevoke() {
  if (confirmingRevoke.value) return revoke()
  confirmingRevoke.value = true
  revokeTimer = setTimeout(() => (confirmingRevoke.value = false), 4000)
}

const revoke = () => run('revoke', async () => {
  if (revokeTimer) clearTimeout(revokeTimer)
  confirmingRevoke.value = false
  if (!share.value) return
  await account.api('DELETE', `/api/share/${share.value.code}`)
  share.value = null
  anonymous.value = null
  await loadPreview()
})

const extend = () => run('extend', async () => {
  if (!share.value) return
  await account.api('POST', `/api/share/${share.value.code}/extend`)
  await loadShare()
})

function expiresIn(expires: number) {
  const ms = expires - Date.now()
  const hours = Math.max(0, Math.round(ms / 3_600_000))
  if (hours < 48) return t('share.expiresHours', { n: hours })
  return t('share.expiresDays', { n: Math.round(hours / 24) })
}

const invite = (userId: string) => run(`inv-${userId}`, async () => {
  if (!share.value) return
  await account.api('POST', `/api/share/${share.value.code}/invite`, { userIds: [userId] })
  await loadShare()
})

const uninvite = (userId: string) => run(`un-${userId}`, async () => {
  if (!share.value) return
  await account.api('DELETE', `/api/share/${share.value.code}/invite`, { userId })
  await loadShare()
})

async function copy(what: 'code' | 'link', value: string) {
  await navigator.clipboard.writeText(value)
  copied.value = what
  setTimeout(() => (copied.value = ''), 1500)
}

const label = (u: { username: string | null, name: string | null }) => u.username || u.name || '—'

let unlistenStage: UnlistenFn | null = null
onMounted(async () => {
  loadPreview()
  loadShare()
  loadFriends()
  unlistenStage = await listen<{ stage: string, current: number, total: number }>(
    'share://progress', e => (progress.value = e.payload))
})
onBeforeUnmount(() => unlistenStage?.())
watch(() => account.isSignedIn.value, () => {
  loadShare()
  loadFriends()
})
</script>

<template>
  <div class="mx-auto max-w-2xl space-y-4 py-2">
    <p v-if="error" class="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-sm text-red-300">
      {{ error }}
    </p>

    <div v-if="!share && !anonymous" class="rounded-xl border border-default p-4">
      <h3 class="text-sm font-semibold">{{ $t('share.tabTitle') }}</h3>
      <p class="mt-1 text-sm text-muted">{{ $t('share.intro') }}</p>

      <div v-if="loadingPreview" class="mt-4 flex items-center gap-2 rounded-lg border border-default px-3 py-4 text-sm text-muted">
        <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin text-primary-400" />
        {{ $t('share.scanning') }}
      </div>

      <div v-else-if="preview" class="mt-4 grid grid-cols-2 gap-2">
        <div class="rounded-lg border border-default p-3">
          <div class="text-xl font-bold">{{ preview.modrinth }}</div>
          <div class="text-xs text-muted">Modrinth</div>
        </div>
        <div class="rounded-lg border border-default p-3">
          <div class="text-xl font-bold">{{ preview.curseforge }}</div>
          <div class="text-xs text-muted">CurseForge</div>
        </div>
      </div>

      <div v-if="preview?.unresolved.length" class="mt-3 rounded-lg border border-default p-3">
        <p class="text-sm font-medium">
          {{ $t('share.extraFiles', { n: preview.unresolved.length }) }}
        </p>
        <p class="mt-0.5 text-xs text-muted">
          {{ $t('share.extraFilesHint', { size: fmtSize(preview.unresolved_bytes) }) }}
        </p>

        <div class="mt-2 max-h-56 overflow-y-auto">
          <div v-for="g in groups" :key="g.dir">
            <div class="flex items-center gap-1.5 rounded-md py-1 pr-2 hover:bg-white/5">
              <button
                type="button"
                class="flex size-4 shrink-0 items-center justify-center text-neutral-500 hover:text-neutral-300"
                @click="toggleExpand(g.dir)"
              >
                <UIcon
                  :name="expanded.has(g.dir) ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
                  class="size-3.5"
                />
              </button>
              <button
                type="button"
                class="flex size-4 shrink-0 items-center justify-center rounded border transition"
                :class="boxClass(groupState(g.files))"
                @click="toggleGroup(g.files)"
              >
                <UIcon v-if="groupState(g.files) === 'checked'" name="i-lucide-check" class="size-3 text-white" />
                <UIcon
                  v-else-if="groupState(g.files) === 'indeterminate'"
                  name="i-lucide-minus"
                  class="size-3 text-white"
                />
              </button>
              <UIcon name="i-lucide-folder" class="size-4 shrink-0 text-neutral-500" />
              <span class="min-w-0 flex-1 truncate text-sm">{{ g.dir }}</span>
              <span class="shrink-0 rounded-full bg-white/8 px-1.5 py-0.5 text-[10px] text-neutral-400">
                {{ g.files.length }}
              </span>
              <span class="w-16 shrink-0 text-right font-mono text-[11px] text-muted">{{ fmtSize(g.bytes) }}</span>
            </div>

            <template v-if="expanded.has(g.dir)">
              <div
                v-for="f in g.files"
                :key="f.path"
                class="flex items-center gap-1.5 rounded-md py-1 pr-2 pl-6 hover:bg-white/5"
              >
                <button
                  type="button"
                  class="flex size-4 shrink-0 items-center justify-center rounded border transition"
                  :class="boxClass(selected.has(f.path) ? 'checked' : 'unchecked')"
                  @click="toggleFile(f.path)"
                >
                  <UIcon v-if="selected.has(f.path)" name="i-lucide-check" class="size-3 text-white" />
                </button>
                <UIcon name="i-lucide-file" class="size-4 shrink-0 text-neutral-500" />
                <span
                  class="min-w-0 flex-1 truncate text-sm"
                  :class="{ 'text-neutral-500 line-through': !selected.has(f.path) }"
                >{{ f.path.split('/').slice(1).join('/') }}</span>
                <span class="w-16 shrink-0 text-right font-mono text-[11px] text-muted">{{ fmtSize(f.size) }}</span>
              </div>
            </template>
          </div>
        </div>

        <p class="mt-2 border-t border-default pt-2 text-xs" :class="selected.size ? 'text-primary-400' : 'text-muted'">
          {{ $t('share.extraFilesSelected', { n: selected.size, size: fmtSize(selectedBytes) }) }}
        </p>
      </div>

      <p class="mt-3 text-xs text-muted">{{ $t('share.privacyNote') }}</p>

      <UButton
        class="mt-4"
        icon="i-lucide-share-2"
        :label="$t('share.createBtn')"
        :loading="busy === 'upload'"
        :disabled="!preview || loadingPreview"
        @click="upload"
      />

      <div v-if="progress" class="mt-4 space-y-2 rounded-lg border border-default bg-elevated/30 p-3">
        <div class="flex items-center gap-2 text-xs">
          <UIcon name="i-lucide-loader-circle" class="size-3.5 shrink-0 animate-spin text-primary-400" />
          <span class="flex-1 text-neutral-200">{{ $t(`share.stage.${progress.stage}`) }}</span>
          <span v-if="stagePercent !== null" class="font-mono text-neutral-400">{{ stagePercent }}%</span>
        </div>
        <div class="h-1.5 overflow-hidden rounded-full bg-white/10">
          <div
            class="h-full rounded-full bg-primary-500 transition-[width] duration-200"
            :class="stagePercent === null && 'w-1/3 animate-pulse'"
            :style="stagePercent !== null ? { width: stagePercent + '%' } : undefined"
          />
        </div>

        <div class="flex items-center gap-2 pt-1 text-[11px] text-muted">
          <span class="flex-1">{{ $t('share.overall') }}</span>
          <span class="font-mono">{{ overallPercent }}%</span>
        </div>
        <div class="h-1 overflow-hidden rounded-full bg-white/8">
          <div
            class="h-full rounded-full bg-primary-400/70 transition-[width] duration-300"
            :style="{ width: overallPercent + '%' }"
          />
        </div>
      </div>

    </div>

    <div v-else class="rounded-xl border border-default p-4">
      <div class="flex flex-wrap items-center gap-3">
        <div class="rounded-lg border border-default bg-elevated/40 px-4 py-2">
          <p class="text-[10px] tracking-wider text-muted uppercase">{{ $t('share.codeLabel') }}</p>
          <p class="font-mono text-2xl font-bold tracking-[0.3em]">{{ code }}</p>
        </div>
        <div class="flex flex-1 flex-wrap gap-2">
          <UButton
            color="neutral"
            variant="soft"
            size="sm"
            :icon="copied === 'code' ? 'i-lucide-check' : 'i-lucide-copy'"
            :label="$t('share.copyCode')"
            @click="copy('code', code)"
          />
          <UButton
            color="neutral"
            variant="soft"
            size="sm"
            :icon="copied === 'link' ? 'i-lucide-check' : 'i-lucide-link'"
            :label="$t('share.copyLink')"
            @click="copy('link', shareUrl)"
          />
        </div>
      </div>

      <div v-if="share" class="mt-4 flex flex-wrap items-center gap-3 border-t border-default pt-4">
        <div class="min-w-[200px] flex-1">
          <p class="text-sm font-medium">{{ $t('share.revision', { n: share.revision }) }}</p>
          <p class="text-xs text-muted">{{ $t('share.pushHint') }}</p>
        </div>
        <UButton
          icon="i-lucide-upload"
          size="sm"
          :label="$t('share.push')"
          :loading="busy === 'upload'"
          @click="upload"
        />
      </div>

      <div v-if="share" class="mt-4 flex flex-wrap items-center gap-3 border-t border-default pt-4">
        <UIcon name="i-lucide-clock" class="size-4 shrink-0 text-neutral-500" />
        <p class="min-w-[180px] flex-1 text-sm" :class="share.canExtend ? 'text-amber-300' : 'text-muted'">
          {{ $t('share.expires', { when: expiresIn(share.expires) }) }}
        </p>
        <UButton
          v-if="share.canExtend"
          size="sm"
          color="neutral"
          variant="soft"
          icon="i-lucide-calendar-plus"
          :label="$t('share.extend')"
          :loading="busy === 'extend'"
          @click="extend"
        />
        <span v-else class="text-xs text-muted">{{ $t('share.extendHint') }}</span>

        <UButton
          size="sm"
          :color="confirmingRevoke ? 'error' : 'neutral'"
          variant="soft"
          :icon="confirmingRevoke ? 'i-lucide-triangle-alert' : 'i-lucide-ban'"
          :label="confirmingRevoke ? $t('share.revokeConfirm') : $t('share.revoke')"
          :loading="busy === 'revoke'"
          @click="askRevoke"
        />
      </div>
      <p v-if="confirmingRevoke" class="mt-2 text-xs text-muted">{{ $t('share.revokeHint') }}</p>

      <div v-if="progress" class="mt-3">
        <div class="flex items-center gap-2 text-xs">
          <UIcon name="i-lucide-loader-circle" class="size-3.5 shrink-0 animate-spin text-primary-400" />
          <span class="flex-1 text-neutral-200">{{ $t(`share.stage.${progress.stage}`) }}</span>
          <span v-if="stagePercent !== null" class="font-mono text-neutral-400">{{ stagePercent }}%</span>
        </div>
        <div class="mt-1.5 h-1.5 overflow-hidden rounded-full bg-white/10">
          <div
            class="h-full rounded-full bg-primary-500 transition-[width] duration-200"
            :class="stagePercent === null && 'w-1/3 animate-pulse'"
            :style="stagePercent !== null ? { width: stagePercent + '%' } : undefined"
          />
        </div>
        <div class="mt-2 flex items-center gap-2 text-[11px] text-muted">
          <span class="flex-1">{{ $t('share.overall') }}</span>
          <span class="font-mono">{{ overallPercent }}%</span>
        </div>
        <div class="mt-1 h-1 overflow-hidden rounded-full bg-white/8">
          <div
            class="h-full rounded-full bg-primary-400/70 transition-[width] duration-300"
            :style="{ width: overallPercent + '%' }"
          />
        </div>
      </div>
    </div>

    <div v-if="!account.isSignedIn.value" class="rounded-xl border border-default p-4 text-center">
      <UIcon name="i-lucide-users" class="mx-auto size-7 text-neutral-600" />
      <p class="mt-2 text-sm font-medium">{{ $t('share.signInTitle') }}</p>
      <p class="mt-1 text-sm text-muted">{{ $t('share.signInHint') }}</p>
      <UButton class="mt-3" size="sm" :label="$t('spectra.signIn')" @click="account.login()" />
    </div>

    <div v-else-if="share" class="rounded-xl border border-default p-4">
      <h3 class="text-sm font-semibold">{{ $t('share.sharedWith') }}</h3>

      <ul v-if="share.recipients.length" class="mt-3 space-y-1.5">
        <li
          v-for="r in share.recipients"
          :key="r.user.id"
          class="group flex items-center gap-2.5 rounded-lg bg-white/4 px-2.5 py-2"
        >
          <img v-if="r.user.image" :src="r.user.image" alt="" class="size-7 rounded-full object-cover">
          <span
            v-else
            class="flex size-7 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
            :style="`background:hsl(${spectraInitial(label(r.user)).hue} 55% 30%)`"
          >{{ spectraInitial(label(r.user)).letter }}</span>
          <span class="flex-1 truncate text-sm">{{ label(r.user) }}</span>
          <span
            v-if="r.importedRevision === null"
            class="rounded-full bg-white/6 px-2 py-0.5 text-[11px] text-neutral-400"
          >{{ $t('share.notInstalled') }}</span>
          <span
            v-else-if="r.outdated"
            class="rounded-full bg-amber-500/15 px-2 py-0.5 text-[11px] text-amber-300"
          >{{ $t('share.outdated') }}</span>
          <span
            v-else
            class="rounded-full bg-emerald-500/15 px-2 py-0.5 text-[11px] text-emerald-300"
          >{{ $t('share.upToDate') }}</span>
          <button
            type="button"
            class="rounded-md p-1 text-neutral-600 opacity-0 transition group-hover:opacity-100 hover:bg-white/5 hover:text-red-400"
            :title="$t('share.uninvite')"
            @click="uninvite(r.user.id)"
          >
            <UIcon name="i-lucide-x" class="size-3.5" />
          </button>
        </li>
      </ul>
      <p v-else class="mt-2 text-sm text-muted">{{ $t('share.nobodyYet') }}</p>

      <div class="mt-4 border-t border-default pt-4">
        <h4 class="text-xs font-semibold tracking-wide text-muted uppercase">{{ $t('share.invite') }}</h4>
        <ul v-if="invitable.length" class="mt-2 space-y-1.5">
          <li
            v-for="f in invitable"
            :key="f.id"
            class="flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 hover:bg-white/4"
          >
            <img v-if="f.image" :src="f.image" alt="" class="size-7 rounded-full object-cover">
            <span
              v-else
              class="flex size-7 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
              :style="`background:hsl(${spectraInitial(label(f)).hue} 55% 30%)`"
            >{{ spectraInitial(label(f)).letter }}</span>
            <span class="flex-1 truncate text-sm">{{ label(f) }}</span>
            <UButton
              size="xs"
              :label="$t('share.inviteBtn')"
              :loading="busy === `inv-${f.id}`"
              @click="invite(f.id)"
            />
          </li>
        </ul>
        <p v-else class="mt-2 text-sm text-muted">{{ $t('share.noFriendsToInvite') }}</p>
      </div>
    </div>
  </div>
</template>
