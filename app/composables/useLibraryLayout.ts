import type { Instance } from '~/types/launcher'

export interface DisplayGroup {
  id: string
  name: string | null
  collapsed: boolean
  items: Instance[]
}

interface RawGroup {
  id: string
  name: string | null
  collapsed?: boolean
  ids: string[]
}

const KEY = 'spectra-library-layout'
const UNGROUPED_ID = '__ungrouped'

function loadRaw(): RawGroup[] {
  if (!import.meta.client) return []
  try {
    return JSON.parse(localStorage.getItem(KEY) || '[]') as RawGroup[]
  } catch {
    return []
  }
}

export const useLibraryLayout = () => {
  const groups = useState<DisplayGroup[]>('library-layout', () => [])

  function persist() {
    if (!import.meta.client) return
    const raw: RawGroup[] = groups.value.map(g => ({
      id: g.id,
      name: g.name,
      collapsed: g.collapsed,
      ids: g.items.map(i => i.id),
    }))
    localStorage.setItem(KEY, JSON.stringify(raw))
  }

  function pinUngroupedLast() {
    const idx = groups.value.findIndex(g => g.name === null)
    if (idx !== -1 && idx !== groups.value.length - 1) {
      const [ung] = groups.value.splice(idx, 1)
      groups.value.push(ung)
    }
  }

  function reconcile(instances: Instance[]) {
    const byId = new Map(instances.map(i => [i.id, i]))
    const used = new Set<string>()
    const next: DisplayGroup[] = []

    for (const g of loadRaw()) {
      const items: Instance[] = []
      for (const id of g.ids) {
        const inst = byId.get(id)
        if (inst && !used.has(id)) {
          items.push(inst)
          used.add(id)
        }
      }
      next.push({ id: g.id, name: g.name ?? null, collapsed: !!g.collapsed, items })
    }

    let ungrouped = next.find(g => g.name === null)
    if (!ungrouped) {
      ungrouped = { id: UNGROUPED_ID, name: null, collapsed: false, items: [] }
      next.push(ungrouped)
    }
    for (const inst of instances) {
      if (!used.has(inst.id)) ungrouped.items.push(inst)
    }

    const idx = next.indexOf(ungrouped)
    if (idx !== next.length - 1) {
      next.splice(idx, 1)
      next.push(ungrouped)
    }

    groups.value = next
    persist()
  }

  function createGroup(name: string) {
    const trimmed = name.trim()
    if (!trimmed) return
    const group: DisplayGroup = { id: crypto.randomUUID(), name: trimmed, collapsed: false, items: [] }
    const ungIdx = groups.value.findIndex(g => g.name === null)
    if (ungIdx === -1) groups.value.push(group)
    else groups.value.splice(ungIdx, 0, group)
    persist()
  }

  function removeGroup(id: string) {
    const i = groups.value.findIndex(g => g.id === id)
    if (i === -1 || groups.value[i].name === null) return
    const [removed] = groups.value.splice(i, 1)
    let ungrouped = groups.value.find(g => g.name === null)
    if (!ungrouped) {
      ungrouped = { id: UNGROUPED_ID, name: null, collapsed: false, items: [] }
      groups.value.push(ungrouped)
    }
    ungrouped.items.push(...removed.items)
    persist()
  }

  function toggleCollapse(id: string) {
    const g = groups.value.find(g => g.id === id)
    if (!g) return
    g.collapsed = !g.collapsed
    persist()
  }

  function onGroupsReordered() {
    pinUngroupedLast()
    persist()
  }

  return { groups, reconcile, persist, createGroup, removeGroup, toggleCollapse, onGroupsReordered }
}
