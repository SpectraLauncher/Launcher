import { defineStore } from 'pinia'

export type ThemeMode = 'dark' | 'oled' | 'squared'

export const ACCENT_COLORS = [
  'sky',
  'blue',
  'indigo',
  'violet',
  'purple',
  'pink',
  'rose',
  'red',
  'orange',
  'amber',
  'green',
  'emerald',
  'teal',
  'cyan',
] as const

export type AccentColor = (typeof ACCENT_COLORS)[number]

const STORAGE_KEY = 'spectra-theme'

interface PersistedTheme {
  mode: ThemeMode
  accent: AccentColor
}

function loadPersisted(): PersistedTheme {
  const fallback: PersistedTheme = { mode: 'dark', accent: 'sky' }
  if (!import.meta.client) return fallback
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return fallback
    const saved = { ...fallback, ...JSON.parse(raw) }
    if ((saved.mode as string) === 'zebatkowo') saved.mode = 'squared'
    return saved
  } catch {
    return fallback
  }
}

export const useThemeStore = defineStore('theme', {
  state: () => loadPersisted() as PersistedTheme,
  getters: {
    bgClass(state): string {
      if (state.mode === 'oled') return 'bg-black'
      return 'bg-primary-950/5'
    },
  },
  actions: {
    persist() {
      if (!import.meta.client) return
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ mode: this.mode, accent: this.accent }),
      )
    },

    apply() {
      if (!import.meta.client) return

      try {
        const colorMode = useColorMode()
        colorMode.preference = 'dark'
      } catch {
        document.documentElement.classList.add('dark')
      }

      document.documentElement.classList.toggle('oled', this.mode === 'oled')
      document.documentElement.classList.toggle('squared', this.mode === 'squared')

      try {
        const appConfig = useAppConfig()
        // @ts-expect-error – ui.colors is augmented by @nuxt/ui
        appConfig.ui.colors.primary = this.accent
      } catch {
      }
    },

    setMode(mode: ThemeMode) {
      this.mode = mode
      this.persist()
      this.apply()
    },

    setAccent(accent: AccentColor) {
      this.accent = accent
      this.persist()
      this.apply()
    },
  },
})
