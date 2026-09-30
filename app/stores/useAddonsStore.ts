import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { Addon, AddonButton, AddonTheme } from '~/types/launcher'

const AVAILABLE_KEY = 'spectra-addons-available'

function cachedAvailable(): boolean {
  try {
    return localStorage.getItem(AVAILABLE_KEY) === 'true'
  } catch {
    return false
  }
}

export type SlotButton = AddonButton & { addonId: string }
export type InstalledTheme = AddonTheme & { addonId: string; addonName: string; key: string }

export const useAddonsStore = defineStore('addons', {
  state: () => ({
    addons: [] as Addon[],
    loaded: false,
    available: false,
    origin: '',
  }),
  getters: {
    active(state): Addon[] {
      return state.available ? state.addons.filter(a => a.enabled && !a.error) : []
    },
    themes(): InstalledTheme[] {
      return this.active.flatMap(a => a.themes.map(t => ({ ...t, addonId: a.id, addonName: a.name, key: `${a.id}:${t.id}` })))
    },
  },
  actions: {
    async load() {
      if (!this.origin) this.origin = await invoke<string>('addon_origin')
      this.addons = await invoke<Addon[]>('addons_list')
      this.loaded = true
    },

    fileUrl(addonId: string, path: string, query: Record<string, string> = {}): string {
      const file = path.split('/').map(encodeURIComponent).join('/')
      const search = new URLSearchParams(query).toString()
      return `${this.origin}/${encodeURIComponent(addonId)}/${file}${search ? `?${search}` : ''}`
    },

    async checkAvailable() {
      const answer = await invoke<boolean | null>('addons_available').catch(() => null)
      this.available = answer ?? cachedAvailable()
      try {
        localStorage.setItem(AVAILABLE_KEY, String(this.available))
      } catch {
      }
      return this.available
    },

    buttons(slot: string): SlotButton[] {
      return this.active.flatMap(a => a.buttons.filter(b => b.slot === slot).map(b => ({ ...b, addonId: a.id })))
    },

    text(addonId: string, raw: string, locale: string): string {
      const match = /^%([^%]+)%$/.exec(raw)
      if (!match) return raw
      const key = match[1]!
      const locales = this.addons.find(a => a.id === addonId)?.locales ?? {}
      return locales[locale]?.[key] ?? locales[locale.split('-')[0]!]?.[key] ?? locales.en?.[key] ?? key
    },

    async setEnabled(id: string, enabled: boolean) {
      await invoke('addons_set_enabled', { id, enabled })
      await this.load()
    },

    async uninstall(id: string) {
      await invoke('addons_uninstall', { id })
      await this.load()
    },

    async reload(id: string) {
      await invoke('addons_reload', { id })
      await this.load()
    },
  },
})
