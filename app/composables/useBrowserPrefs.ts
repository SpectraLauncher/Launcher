import type { ModrinthHit, ModrinthSortIndex } from '~/types/modrinth'

export type BrowserView = 'grid' | 'list'
export type BrowserDensity = 'cosy' | 'compact'

const PREFS_KEY = 'spectra-content-browser'
const RECENT_LIMIT = 12

export function useBrowserPrefs() {
  const view = ref<BrowserView>('grid')
  const density = ref<BrowserDensity>('cosy')
  const sort = ref<ModrinthSortIndex>('relevance')
  const recent = ref<ModrinthHit[]>([])

  function loadPrefs() {
    try {
      const raw = localStorage.getItem(PREFS_KEY)
      if (!raw) return
      const saved = JSON.parse(raw) as {
        view?: BrowserView
        density?: BrowserDensity
        sort?: ModrinthSortIndex
        recent?: ModrinthHit[]
      }
      if (saved.view === 'grid' || saved.view === 'list') view.value = saved.view
      if (saved.density === 'cosy' || saved.density === 'compact') density.value = saved.density
      if (saved.sort) sort.value = saved.sort
      if (Array.isArray(saved.recent)) recent.value = saved.recent.slice(0, RECENT_LIMIT)
    } catch {
      void 0
    }
  }

  function savePrefs() {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({
        view: view.value,
        density: density.value,
        sort: sort.value,
        recent: recent.value.slice(0, RECENT_LIMIT),
      }))
    } catch {
      void 0
    }
  }

  function rememberRecent(hit: ModrinthHit) {
    recent.value = [hit, ...recent.value.filter(r => r.project_id !== hit.project_id)].slice(0, RECENT_LIMIT)
  }

  watch([view, density, sort, recent], savePrefs, { deep: true })

  return { view, density, sort, recent, loadPrefs, savePrefs, rememberRecent }
}
