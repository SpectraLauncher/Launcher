import { defineStore } from 'pinia'

export type ThemeMode = 'dark' | 'oled' | 'squared'

/** Accent colors offered in settings — these are @nuxt/ui color aliases. */
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
    // The old gold "Zębatkowo" theme became the accent-agnostic "Squared".
    if ((saved.mode as string) === 'zebatkowo') saved.mode = 'squared'
    return saved
  } catch {
    return fallback
  }
}

export const useThemeStore = defineStore('theme', {
  state: () => loadPersisted() as PersistedTheme,
  getters: {
    /** Root background class for the app shell (matches the Spectra design). */
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

    /** Applies the current theme to the document. Safe to call repeatedly. */
    apply() {
      if (!import.meta.client) return

      // This launcher is dark-only; OLED is a darker variant of dark.
      try {
        const colorMode = useColorMode()
        colorMode.preference = 'dark'
      } catch {
        document.documentElement.classList.add('dark')
      }

      document.documentElement.classList.toggle('oled', this.mode === 'oled')
      // "Squared": same palette, no rounded corners — driven entirely by CSS
      // scoped to this class (see main.css), so the accent still applies.
      document.documentElement.classList.toggle('squared', this.mode === 'squared')

      // Live accent change via @nuxt/ui app config.
      try {
        const appConfig = useAppConfig()
        // @ts-expect-error – ui.colors is augmented by @nuxt/ui
        appConfig.ui.colors.primary = this.accent
      } catch {
        // app config not ready yet; will be applied on next call
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
