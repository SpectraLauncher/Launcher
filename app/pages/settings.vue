<template>
  <div class="h-full flex flex-col p-6 lg:p-8">
    <div class="mx-auto w-full max-w-5xl flex flex-1 min-h-0 flex-col gap-6">
      <h1 class="shrink-0 text-2xl font-bold tracking-tight">{{ t('settings.title') }}</h1>

      <div class="grid min-h-0 flex-1 grid-cols-1 gap-6 md:grid-cols-[220px_1fr]">
        <nav class="flex flex-col gap-1 relative">
          <button
            v-for="s in sections"
            :key="s.key"
            type="button"
            class="flex items-center text-left gap-2 rounded-lg px-3 py-2 text-sm font-medium transition"
            :class="section === s.key ? 'bg-primary-500/15 text-primary-400' : 'text-neutral-400 hover:bg-white/5 hover:text-neutral-200'"
            @click="section = s.key"
          >
            <UIcon :name="s.icon" class="size-4" />
            {{ t(`settings.sections.${s.key}`) }}
          </button>

        </nav>

        <div class="min-w-0 min-h-0 space-y-6 overflow-y-auto px-2">
          <template v-if="section === 'appearance'">
            <div>
              <p class="text-sm font-medium">{{ t('settings.appearance.theme') }}</p>
              <p class="mb-2 text-xs text-muted">{{ t('settings.appearance.themeDesc') }}</p>
              <div class="flex gap-3">
                <button
                  v-for="opt in themeOptions"
                  :key="opt.value"
                  type="button"
                  class="flex-1 rounded-lg border px-4 py-3 text-left transition"
                  :class="theme.mode === opt.value ? 'border-primary-500 ring-1 ring-primary-500 bg-primary-500/10' : 'border-default hover:border-neutral-500'"
                  @click="theme.setMode(opt.value)"
                >
                  <span class="block text-sm font-medium">{{ opt.label }}</span>
                  <span class="mt-2 block h-6 rounded" :style="{ background: opt.preview }" />
                </button>
              </div>
            </div>
            <div>
              <p class="text-sm font-medium">{{ t('settings.appearance.accent') }}</p>
              <p class="mb-2 text-xs text-muted">{{ t('settings.appearance.accentDesc') }}</p>
              <div class="flex flex-wrap gap-2">
                <button
                  v-for="c in ACCENT_COLORS"
                  :key="c"
                  type="button"
                  :title="c"
                  class="size-7 rounded-full transition"
                  :class="theme.accent === c ? 'ring-2 ring-white ring-offset-2 ring-offset-neutral-950' : 'hover:scale-110'"
                  :style="{ background: accentHex[c] }"
                  @click="theme.setAccent(c)"
                />
              </div>
            </div>
            <div v-if="sponsor.sponsors.length > 0" class="flex items-center justify-between gap-4">
              <div>
                <p class="text-sm font-medium">{{ t('settings.appearance.sponsor') }}</p>
                <p class="text-xs text-muted">{{ t('settings.appearance.sponsorDesc') }}</p>
              </div>
              <USwitch
                :model-value="!sponsor.dismissed.value"
                @update:model-value="(v: boolean) => v ? sponsor.restore() : sponsor.dismiss()"
              />
            </div>
          </template>

          <template v-else-if="section === 'language'">
            <UFormField :label="t('settings.language.title')" :description="t('settings.language.auto')">
              <USelect :model-value="locale" :items="localeItems" value-key="value" class="w-56" @update:model-value="onLocaleChange" />
            </UFormField>
          </template>

          <template v-else-if="section === 'privacy' && settings">
            <div
              v-for="opt in privacyOptions"
              :key="opt.key"
              class="flex items-center justify-between gap-4"
            >
              <div>
                <p class="flex items-center gap-2 text-sm font-medium">
                  {{ t(`settings.privacy.${opt.key}`) }}
                  <UBadge v-if="opt.wip" color="neutral" variant="subtle" size="xs" label="WIP" />
                </p>
                <p class="text-xs text-muted">{{ t(`settings.privacy.${opt.key}Desc`) }}</p>
              </div>
              <USwitch v-model="settings[opt.key]" />
            </div>
          </template>

          <template v-else-if="section === 'java'">
            <div class="flex items-start justify-between gap-3">
              <div>
                <p class="text-sm font-medium">{{ t('settings.java.detected') }}</p>
                <p class="text-xs text-muted">{{ t('settings.java.detectedDesc') }}</p>
              </div>
              <UButton icon="i-lucide-radar" size="xs" variant="soft" :loading="java.scanning.value" :label="t('settings.java.autoDetect')" @click="java.scan()" />
            </div>

            <div v-if="!java.installations.value.length && !java.scanning.value" class="py-8 text-center text-sm text-muted">
              {{ t('settings.java.noJava') }}
            </div>
            <div v-else class="space-y-2">
              <div
                v-for="inst in java.installations.value"
                :key="inst.path"
                class="flex items-center gap-3 rounded-xl border border-default bg-white/3 p-3"
              >
                <UIcon name="i-lucide-coffee" class="size-5 shrink-0 text-primary-400" />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="font-medium">Java {{ inst.major ?? '?' }}</span>
                    <span class="font-mono text-[11px] text-neutral-500">{{ inst.version }}</span>
                    <span v-if="inst.vendor" class="text-[11px] text-neutral-500">· {{ inst.vendor }}</span>
                  </div>
                  <div class="truncate font-mono text-[11px] text-neutral-500" :title="inst.path">{{ inst.path }}</div>
                </div>
                <UBadge v-if="settings && settings.default_java_path === inst.path" color="success" variant="subtle" size="xs" :label="t('settings.java.isDefault')" />
                <UButton v-else size="xs" color="neutral" variant="soft" :label="t('settings.java.setDefault')" @click="setDefaultJava(inst.path)" />
              </div>
            </div>

            <UFormField v-if="settings" :label="t('settings.java.defaultPath')" :description="t('settings.java.defaultPathHint')">
              <div class="flex gap-2">
                <UInput v-model="settings.default_java_path" placeholder="javaw.exe" class="flex-1 font-mono text-xs" />
                <UButton icon="i-lucide-folder" color="neutral" variant="soft" @click="browseDefaultJava" />
              </div>
            </UFormField>
          </template>

          <template v-else-if="section === 'defaults' && settings">
            <UFormField :label="t('settings.defaults.memory')" :description="t('settings.defaults.memoryDesc')">
              <div class="flex items-center gap-3 pt-1">
                <USlider v-model="settings.default_memory_mb" :min="sysMem.minMb" :max="sysMem.maxMb.value" :step="256" class="flex-1" />
                <span class="w-24 shrink-0 text-right font-mono text-sm">{{ settings.default_memory_mb }} MB</span>
              </div>
            </UFormField>

            <div class="flex items-center justify-between gap-4">
              <div>
                <p class="text-sm font-medium">{{ t('instSettings.fullscreen') }}</p>
                <p class="text-xs text-muted">{{ t('instSettings.fullscreenDesc') }}</p>
              </div>
              <USwitch v-model="settings.default_fullscreen" />
            </div>
            <div class="flex gap-4">
              <UFormField :label="t('instSettings.width')" :description="t('instSettings.widthDesc')">
                <UInput v-model.number="settings.default_width" type="number" placeholder="854" class="w-36" />
              </UFormField>
              <UFormField :label="t('instSettings.height')" :description="t('instSettings.heightDesc')">
                <UInput v-model.number="settings.default_height" type="number" placeholder="480" class="w-36" />
              </UFormField>
            </div>

            <UFormField :label="t('instSettings.customJavaArgs')" :description="t('instSettings.customJavaArgsDesc')">
              <UTextarea v-model="defJavaArgs" :rows="2" placeholder="-Xmx4G -XX:+UseG1GC" class="w-full font-mono text-xs" />
            </UFormField>

            <div>
              <p class="text-sm font-medium">{{ t('instSettings.customEnv') }}</p>
              <p class="mb-2 text-xs text-muted">{{ t('instSettings.customEnvDesc') }}</p>
              <div class="space-y-2">
                <div v-for="(e, i) in settings.default_env_vars" :key="i" class="flex gap-2">
                  <UInput v-model="e.key" placeholder="KEY" class="flex-1" />
                  <UInput v-model="e.value" placeholder="value" class="flex-1" />
                  <UButton icon="i-lucide-x" color="neutral" variant="ghost" square @click="settings.default_env_vars.splice(i, 1)" />
                </div>
                <UButton icon="i-lucide-plus" size="xs" variant="soft" :label="t('instSettings.addEnv')" @click="settings.default_env_vars.push({ key: '', value: '' })" />
              </div>
            </div>

            <UFormField :label="t('instSettings.preLaunch')" :description="t('instSettings.preLaunchHint')">
              <UInput v-model="settings.default_pre_launch" class="w-full font-mono text-xs" />
            </UFormField>
            <UFormField :label="t('instSettings.wrapper')" :description="t('instSettings.wrapperHint')">
              <UInput v-model="settings.default_wrapper" class="w-full font-mono text-xs" />
            </UFormField>
            <UFormField :label="t('instSettings.postExit')" :description="t('instSettings.postExitHint')">
              <UInput v-model="settings.default_post_exit" class="w-full font-mono text-xs" />
            </UFormField>
          </template>

          <template v-else-if="section === 'accounts'">
            <p v-if="!accounts.accounts.length" class="text-sm text-muted">{{ t('settings.accounts.noAccounts') }}</p>
            <ul v-else class="space-y-2">
              <li
                v-for="acc in accounts.accounts"
                :key="acc.uuid"
                class="flex items-center gap-3 rounded-xl border border-default px-3 py-2"
              >
                <UAvatar :src="avatarUrl(acc)" :alt="acc.username" size="sm" />
                <div class="min-w-0 flex-1">
                  <p class="truncate text-sm font-medium">{{ acc.username }}</p>
                  <p class="text-xs text-neutral-500 capitalize">{{ acc.kind }}</p>
                </div>
                <UBadge v-if="acc.uuid === accounts.activeUuid" color="primary" variant="subtle" :label="t('settings.accounts.active')" />
                <UButton v-else size="xs" variant="ghost" :label="t('settings.accounts.setActive')" @click="accounts.setActive(acc.uuid)" />
                <UButton icon="i-lucide-trash-2" size="xs" color="error" variant="ghost" @click="accounts.remove(acc.uuid)" />
              </li>
            </ul>

            <div class="flex flex-col gap-3 border-t border-default pt-3">
              <UButton icon="i-lucide-log-in" :label="t('settings.accounts.microsoft')" :loading="accounts.loading" class="w-fit" @click="onMicrosoftLogin" />
              <form class="flex items-end gap-2" @submit.prevent="onOfflineLogin">
                <UFormField :label="t('settings.accounts.offlineUsername')" class="flex-1">
                  <UInput v-model="offlineName" placeholder="Steve" />
                </UFormField>
                <UButton type="submit" variant="soft" icon="i-lucide-user-plus" :label="t('settings.accounts.offline')" :disabled="offlineName.trim().length < 3" />
              </form>
              <p class="text-xs text-muted">{{ t('settings.accounts.offlineHint') }}</p>
              <p v-if="accounts.error" class="text-xs text-error">{{ accounts.error }}</p>
            </div>
          </template>
        </div>
      </div>
    </div>
    <div class="shrink-0 mx-auto w-full max-w-5xl flex gap-3 items-center border-t border-default pt-4 mt-4">
        <img src="/logo-transparent.png" alt="Spectra Launcher Icon" class="w-8 h-8 rounded-sm" />
        <div>
          <p>Spectra Launcher v{{ version }}</p>
          <p class="text-xs text-muted">Made with by <a href="https://github.com/MakotoPD" target="_blank" rel="noopener noreferrer">MakotoPD</a></p>
        </div>
        <UButton
          v-if="updater.available.value"
          color="primary"
          icon="i-lucide-download"
          :loading="updater.status.value === 'downloading' || updater.status.value === 'ready'"
          :label="updater.status.value === 'ready'
            ? t('update.ready')
            : updater.status.value === 'downloading'
              ? t('update.downloading', { progress: updater.progress.value })
              : t('update.available', { version: updater.newVersion.value })"
          @click="updater.downloadAndInstall()"
        />
        <UButton
          v-else
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-lucide-refresh-cw"
          :loading="updater.status.value === 'checking'"
          :label="updater.status.value === 'uptodate' ? t('update.uptodate') : t('update.check')"
          @click="updater.checkForUpdates(false)"
        />
      </div>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { open } from '@tauri-apps/plugin-dialog'
import { ACCENT_COLORS, type ThemeMode } from '~/stores/useThemeStore'
import type { Account, Settings } from '~/types/launcher'

const { t, locale, locales, setLocale } = useI18n()
const theme = useThemeStore()
const sponsor = useSponsor()
const accounts = useAccountStore()
const java = useJava()
const sysMem = useSystemMemory()
const updater = useAutoUpdate()

type Section = 'appearance' | 'language' | 'privacy' | 'java' | 'defaults' | 'accounts'
const sections: { key: Section; icon: string }[] = [
  { key: 'appearance', icon: 'i-lucide-palette' },
  { key: 'language', icon: 'i-lucide-languages' },
  { key: 'privacy', icon: 'i-lucide-shield' },
  { key: 'java', icon: 'i-lucide-coffee' },
  { key: 'defaults', icon: 'i-lucide-sliders-horizontal' },
  { key: 'accounts', icon: 'i-lucide-users' },
]
const section = ref<Section>('appearance')

type PrivacyKey = 'track_playtime' | 'share_activity' | 'discord_rpc' | 'crash_reports' | 'anonymous_stats'
const privacyOptions: { key: PrivacyKey; wip?: boolean }[] = [
  { key: 'track_playtime' },
  { key: 'share_activity' },
  { key: 'discord_rpc' },
  { key: 'crash_reports', wip: true },
  { key: 'anonymous_stats' },
]

const offlineName = ref('')
const settings = ref<Settings | null>(null)
const version = ref('')

onMounted(async () => {
  accounts.load()
  java.scan()
  sysMem.ensure()
  getVersion().then((v) => { version.value = v })
  try {
    settings.value = await invoke<Settings>('get_settings')
  } catch {  }
})

const defJavaArgs = computed({
  get: () => settings.value?.default_java_args.join(' ') ?? '',
  set: (v: string) => { if (settings.value) settings.value.default_java_args = v.split(/\s+/).filter(Boolean) },
})

function setDefaultJava(path: string) {
  if (settings.value) settings.value.default_java_path = path
}

async function browseDefaultJava() {
  const p = await open({ multiple: false, directory: false })
  if (typeof p === 'string' && settings.value) settings.value.default_java_path = p
}

let saveTimer: ReturnType<typeof setTimeout> | undefined
watch(settings, () => {
  if (!settings.value) return
  clearTimeout(saveTimer)
  const snapshot = JSON.parse(JSON.stringify(settings.value)) as Settings
  saveTimer = setTimeout(() => invoke('save_settings', { settings: snapshot }).catch(() => {}), 500)
}, { deep: true })

const telemetry = useTelemetry()
watch(() => settings.value?.anonymous_stats, (on) => {
  if (on !== undefined) telemetry.setEnabled(on)
})

const themeOptions = computed<{ value: ThemeMode; label: string; preview: string }[]>(() => [
  { value: 'dark', label: t('settings.appearance.themeDark'), preview: '#0a0a0b' },
  { value: 'oled', label: t('settings.appearance.themeOled'), preview: '#000000' },
  { value: 'squared', label: t('settings.appearance.themeSquared'), preview: 'linear-gradient(135deg,#0a0a0b 50%,#52525b 50%)' },
])

const accentHex: Record<string, string> = {
  sky: '#0ea5e9', blue: '#3b82f6', indigo: '#6366f1', violet: '#8b5cf6',
  purple: '#a855f7', pink: '#ec4899', rose: '#f43f5e', red: '#ef4444',
  orange: '#f97316', amber: '#f59e0b', green: '#22c55e', emerald: '#10b981',
  teal: '#14b8a6', cyan: '#06b6d4',
}

const localeItems = computed(() =>
  (locales.value as { code: string; name?: string }[]).map(l => ({ label: l.name ?? l.code, value: l.code })),
)
const onLocaleChange = (code: string) => setLocale(code as 'en' | 'pl' | 'de' | 'es' | 'fr' | 'zh' | 'ru')

const avatarUrl = (acc: Account) =>
  acc.kind === 'microsoft' ? `https://crafatar.com/avatars/${acc.uuid}?overlay` : undefined

const onMicrosoftLogin = async () => {
  try { await accounts.login() } catch {  }
}
const onOfflineLogin = async () => {
  const name = offlineName.value.trim()
  if (name.length < 3) return
  try {
    await accounts.loginOffline(name)
    offlineName.value = ''
  } catch {  }
}
</script>
