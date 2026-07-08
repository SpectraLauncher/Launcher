/**
 * Sponsored content shown on the home page.
 *
 * HOW TO ADD / REMOVE SPONSORS
 * ─────────────────────────────
 * Edit the SPONSORS array below. Each entry has a `type` field:
 *
 *   • 'modpack'  – a Modrinth/CurseForge modpack
 *   • 'server'   – a Minecraft server
 *   • 'hosting'  – a Minecraft hosting provider
 *
 * Leave the array empty ([]) to hide the sponsored section entirely.
 *
 * Example entry:
 *
 *   {
 *     type: 'modpack',
 *     id: 'my-modpack',
 *     title: 'My Modpack',
 *     description: 'A short description shown on the card.',
 *     iconUrl: 'https://example.com/icon.png',
 *     url: 'https://modrinth.com/modpack/my-modpack',
 *     // Optional: for Modrinth modpacks — opens the browser pre-filtered
 *     modrinthSlug: 'my-modpack',
 *   },
 *
 *   {
 *     type: 'server',
 *     id: 'my-server',
 *     title: 'My Server',
 *     description: 'Join us at play.myserver.net!',
 *     iconUrl: 'https://example.com/server-icon.png',
 *     url: 'https://myserver.net',
 *     address: 'play.myserver.net',   // shown as a copy-able address
 *   },
 *
 *   {
 *     type: 'hosting',
 *     id: 'my-hosting',
 *     title: 'My Hosting',
 *     description: 'The best Minecraft hosting.',
 *     iconUrl: 'https://example.com/logo.png',
 *     url: 'https://myhosting.net',
 *   },
 */


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

// ─── SPONSORS ────────────────────────────────────────────────────────────────

export const SPONSORS: Sponsor[] = [
  // Add your sponsored entries here.
  // Empty array = sponsored section is hidden.
]

// ─── Composable ───────────────────────────────────────────────────────────────

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
