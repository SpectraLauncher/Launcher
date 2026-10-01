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

const TINT_PALETTES = ['neutral', 'zinc', 'gray', 'slate', 'stone']
const TINT_SHADES: Record<string, string> = {
  50: 'white 20%',
  100: 'white 10%',
  200: 'black 0%',
  300: 'black 12%',
  400: 'black 25%',
  500: 'black 38%',
  600: 'black 50%',
  700: 'black 62%',
  800: 'black 72%',
  900: 'black 84%',
  950: 'black 92%',
}

export function tintVariables(tint: string): Record<string, string> {
  const mix = (other: string) => `color-mix(in oklab, ${tint}, ${other})`
  const vars: Record<string, string> = {
    '--color-white': mix('white 10%'),
    '--mk-text': mix('white 10%'),
    '--mk-placeholder': mix('black 50%'),
    '--ui-text-highlighted': mix('white 10%'),
    '--ui-bg-inverted': mix('white 10%'),
    '--ui-border-inverted': mix('white 10%'),
  }
  for (const palette of TINT_PALETTES) {
    for (const [shade, other] of Object.entries(TINT_SHADES)) vars[`--color-${palette}-${shade}`] = mix(other)
  }
  return vars
}

const TINT_NAMES = Object.keys(tintVariables('#000000'))

interface PersistedTheme {
  mode: ThemeMode
  accent: AccentColor
  addonTheme: string | null
  tint: string | null
}

function loadPersisted(): PersistedTheme {
  const fallback: PersistedTheme = { mode: 'dark', accent: 'sky', addonTheme: null, tint: null }
  if (!import.meta.client) return fallback
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return fallback
    const saved = { ...fallback, ...JSON.parse(raw) }
    if ((saved.mode as string) === 'zebatkowo') saved.mode = 'squared'
    if (typeof saved.tint !== 'string' || !/^#[0-9a-f]{6}$/i.test(saved.tint)) saved.tint = null
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
        JSON.stringify({ mode: this.mode, accent: this.accent, addonTheme: this.addonTheme, tint: this.tint }),
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

      const style = document.documentElement.style
      for (const name of TINT_NAMES) style.removeProperty(name)
      if (this.tint) {
        for (const [name, value] of Object.entries(tintVariables(this.tint))) style.setProperty(name, value)
      }

      try {
        const appConfig = useAppConfig()
        appConfig.ui.colors.primary = this.accent
      } catch {
      }
    },

    setMode(mode: ThemeMode) {
      this.mode = mode
      this.addonTheme = null
      this.tint = null
      this.persist()
      this.apply()
    },

    syncTint(tint: string | null) {
      if (this.tint === tint) return
      this.tint = tint
      this.persist()
      this.apply()
    },

    applyAddonTheme(theme: { key: string; mode: ThemeMode | null; accent: string | null; tint?: string | null }) {
      if (theme.mode) this.mode = theme.mode
      this.tint = theme.tint ?? null
      if (theme.accent && (ACCENT_COLORS as readonly string[]).includes(theme.accent)) {
        this.accent = theme.accent as AccentColor
      }
      this.addonTheme = theme.key
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
