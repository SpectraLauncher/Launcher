<template>
  <UModal v-model:open="open" :dismissible="false" :ui="{ content: 'max-w-lg' }">
    <template #content>
      <div class="flex flex-col">
        <div class="flex items-center justify-center gap-1.5 border-b border-default py-3">
          <span
            v-for="i in steps"
            :key="i"
            class="h-1.5 rounded-full transition-all"
            :class="i - 1 === step ? 'w-6 bg-primary-500' : 'w-1.5 bg-white/15'"
          />
        </div>

        <div class="min-h-[300px] px-6 py-6">
          <div v-if="step === 0" class="flex flex-col items-center gap-4 text-center">
            <img src="/logo.png" alt="" class="size-16 rounded-2xl" >
            <div>
              <h2 class="text-xl font-bold">{{ $t('onboarding.welcomeTitle') }}</h2>
              <p class="mt-2 text-sm text-muted">{{ $t('onboarding.welcomeText') }}</p>
            </div>
          </div>

          <div v-else-if="step === 1" class="space-y-4">
            <div>
              <h2 class="text-lg font-semibold">{{ $t('onboarding.accountTitle') }}</h2>
              <p class="mt-1 text-sm text-muted">{{ $t('onboarding.accountText') }}</p>
            </div>
            <div v-if="accounts.activeAccount" class="flex items-center gap-2 rounded-lg border border-emerald-500/40 bg-emerald-500/10 p-3 text-sm">
              <UIcon name="i-lucide-circle-check" class="size-4 text-emerald-400" />
              {{ $t('onboarding.signedInAs', { name: accounts.activeAccount.username }) }}
            </div>
            <template v-else>
              <UButton block icon="i-lucide-user" :loading="loggingIn" :label="$t('settings.accounts.microsoft')" @click="msLogin" />
              <div class="flex gap-2">
                <UInput v-model="offlineName" :placeholder="$t('settings.accounts.offlineUsername')" class="flex-1" />
                <UButton color="neutral" variant="soft" :disabled="!offlineName.trim()" :label="$t('settings.accounts.offline')" @click="offlineLogin" />
              </div>
            </template>
          </div>

          <div v-else-if="step === 2" class="space-y-4">
            <div>
              <h2 class="text-lg font-semibold">{{ $t('onboarding.themeTitle') }}</h2>
              <p class="mt-1 text-sm text-muted">{{ $t('onboarding.themeText') }}</p>
            </div>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="opt in themeOptions"
                :key="opt.value"
                type="button"
                class="rounded-lg border px-3 py-2.5 text-left transition"
                :class="theme.mode === opt.value ? 'border-primary-500 ring-1 ring-primary-500 bg-primary-500/10' : 'border-default hover:border-neutral-500'"
                @click="theme.setMode(opt.value)"
              >
                <span class="block text-xs font-medium">{{ opt.label }}</span>
                <span class="mt-2 block h-5 rounded" :style="{ background: opt.preview }" />
              </button>
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                v-for="c in ACCENT_COLORS"
                :key="c"
                type="button"
                class="size-6 rounded-full transition"
                :class="theme.accent === c ? 'ring-2 ring-white ring-offset-2 ring-offset-neutral-950' : 'hover:scale-110'"
                :style="{ background: accentHex[c] }"
                @click="theme.setAccent(c)"
              />
            </div>
          </div>

          <div v-else-if="step === 3" class="space-y-5">
            <div>
              <h2 class="text-lg font-semibold">{{ $t('onboarding.javaTitle') }}</h2>
              <p class="mt-1 text-sm text-muted">{{ $t('onboarding.javaText') }}</p>
            </div>
            <div class="flex items-center justify-between rounded-lg border border-default p-3 text-sm">
              <span class="flex items-center gap-2">
                <UIcon name="i-lucide-coffee" class="size-4 text-neutral-400" />
                {{ java.scanning.value ? $t('settings.java.scanning') : $t('onboarding.javaFound', { n: java.installations.value.length }) }}
              </span>
              <UButton size="xs" color="neutral" variant="soft" :loading="java.scanning.value" :label="$t('settings.java.autoDetect')" @click="java.scan()" />
            </div>
            <div>
              <div class="mb-1 flex items-center justify-between text-sm">
                <span class="font-medium">{{ $t('onboarding.ram') }}</span>
                <span class="font-mono text-primary-300">{{ (ram / 1024).toFixed(1) }} GB</span>
              </div>
              <USlider v-model="ram" :min="sysMem.minMb" :max="sysMem.maxMb.value" :step="256" />
            </div>
          </div>

          <div v-else-if="step === 4" class="space-y-4">
            <div>
              <h2 class="text-lg font-semibold">{{ $t('onboarding.privacyTitle') }}</h2>
              <p class="mt-1 text-sm text-muted">{{ $t('onboarding.privacyText') }}</p>
            </div>
            <div
              v-for="opt in privacyOptions"
              :key="opt.key"
              class="flex items-start justify-between gap-4 rounded-lg border border-default p-3"
            >
              <div class="min-w-0">
                <p class="flex items-center gap-2 text-sm font-medium">
                  {{ $t(`settings.privacy.${opt.key}`) }}
                  <UBadge v-if="opt.wip" color="neutral" variant="subtle" size="xs" label="WIP" />
                </p>
                <p class="mt-0.5 text-xs text-muted">{{ $t(`settings.privacy.${opt.key}Desc`) }}</p>
              </div>
              <USwitch v-if="settings" v-model="settings[opt.key]" class="shrink-0 mt-0.5" />
            </div>
          </div>

          <div v-else class="flex flex-col items-center gap-4 py-6 text-center">
            <UIcon name="i-lucide-party-popper" class="size-12 text-primary-400" />
            <div>
              <h2 class="text-xl font-bold">{{ $t('onboarding.doneTitle') }}</h2>
              <p class="mt-2 text-sm text-muted">{{ $t('onboarding.doneText') }}</p>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-between border-t border-default px-6 py-3">
          <UButton v-if="step > 0 && step < steps - 1" variant="ghost" color="neutral" :label="$t('create.back')" @click="step--" />
          <UButton v-else variant="ghost" color="neutral" :label="$t('onboarding.skip')" @click="finish" />
          <UButton v-if="step < steps - 1" :label="$t('onboarding.next')" trailing-icon="i-lucide-arrow-right" @click="step++" />
          <UButton v-else icon="i-lucide-check" :label="$t('onboarding.start')" @click="complete" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { ACCENT_COLORS, type ThemeMode } from '~/stores/useThemeStore'
import type { Settings } from '~/types/launcher'

const { open, isDone, finish } = useOnboarding()
const accounts = useAccountStore()
const theme = useThemeStore()
const java = useJava()
const sysMem = useSystemMemory()
const toast = useToast()
const { t } = useI18n()

const steps = 6
const step = ref(0)
const offlineName = ref('')
const loggingIn = ref(false)
const ram = ref(4096)
const settings = ref<Settings | null>(null)

type PrivacyKey = 'track_playtime' | 'share_activity' | 'discord_rpc' | 'crash_reports' | 'anonymous_stats'
const privacyOptions: { key: PrivacyKey; wip?: boolean }[] = [
  { key: 'track_playtime' },
  { key: 'share_activity' },
  { key: 'discord_rpc' },
  { key: 'anonymous_stats' },
  { key: 'crash_reports', wip: true },
]

const themeOptions = computed<{ value: ThemeMode, label: string, preview: string }[]>(() => [
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

onMounted(async () => {
  if (!isDone()) open.value = true
  await accounts.load()
  await sysMem.ensure()
  try {
    settings.value = await invoke<Settings>('get_settings')
    ram.value = settings.value.default_memory_mb || 4096
    settings.value.track_playtime = settings.value.track_playtime ?? true
    settings.value.discord_rpc = settings.value.discord_rpc ?? true
    settings.value.crash_reports = settings.value.crash_reports ?? true
    settings.value.anonymous_stats = settings.value.anonymous_stats ?? true
    settings.value.share_activity = settings.value.share_activity ?? true
  } catch {  }
})

watch(open, (v) => {
  if (v) {
    step.value = 0
    if (!java.installations.value.length) java.scan()
  }
})

async function msLogin() {
  loggingIn.value = true
  try { await accounts.login() }
  catch (e) { toast.add({ title: String(e), color: 'error' }) }
  finally { loggingIn.value = false }
}
async function offlineLogin() {
  const name = offlineName.value.trim()
  if (!name) return
  try { await accounts.loginOffline(name) }
  catch (e) { toast.add({ title: String(e), color: 'error' }) }
}

async function complete() {
  if (settings.value) {
    try {
      await invoke('save_settings', { settings: { ...settings.value, default_memory_mb: ram.value } })
    } catch {  }
  }
  finish()
}
</script>
