export type SponsorType = 'modpack' | 'server' | 'hosting'

export interface SponsorBase {
  type: SponsorType
  id: string
  title: string
  description: string
  iconUrl: string
  url: string
}

export interface ModpackSponsor extends SponsorBase {
  type: 'modpack'
  modrinthSlug?: string
}

export interface ServerSponsor extends SponsorBase {
  type: 'server'
  address?: string
}

export interface HostingSponsor extends SponsorBase {
  type: 'hosting'
}

export type Sponsor = ModpackSponsor | ServerSponsor | HostingSponsor

export const SPONSORS: Sponsor[] = [
]

const KEY = 'spectra-sponsor-hidden'

export const useSponsor = () => {
  const dismissed = useState('sponsor-dismissed', () => {
    if (!import.meta.client) return false
    return localStorage.getItem(KEY) === '1'
  })

  function dismiss() {
    dismissed.value = true
    if (import.meta.client) localStorage.setItem(KEY, '1')
  }

  function restore() {
    dismissed.value = false
    if (import.meta.client) localStorage.removeItem(KEY)
  }

  const visible = computed(() => SPONSORS.length > 0 && !dismissed.value)

  return { sponsors: SPONSORS, dismissed, dismiss, restore, visible }
}
