import { invoke } from '@tauri-apps/api/core'

export interface SpectraUser {
  id: string
  name: string | null
  username: string | null
  image: string | null
  email?: string
  mcUsername?: string | null
  mcUuid?: string | null
}

export type FriendStatus = 'online' | 'in_game' | 'dnd' | 'offline'
export type PresenceMode = 'visible' | 'dnd' | 'hidden'

export interface SpectraFriend {
  id: string
  name: string | null
  username: string | null
  image: string | null
  mcUsername?: string | null
  friendshipId: number
  status: FriendStatus
}

export interface FriendRequest {
  id: number
  created: number
  user: Omit<SpectraFriend, 'friendshipId'>
}

export const useSpectraAccount = () => {
  const user = useState<SpectraUser | null>('spectra-user', () => null)
  const loading = useState('spectra-loading', () => false)
  const error = useState<string | null>('spectra-error', () => null)

  const isSignedIn = computed(() => user.value != null)

  async function api<T>(method: 'GET' | 'POST' | 'PATCH' | 'DELETE', path: string, body?: unknown) {
    return invoke<T>('spectra_api', { method, path, body: body ?? null })
  }

  async function refresh() {
    loading.value = true
    try {
      user.value = await invoke<SpectraUser | null>('spectra_session')
    } catch {
      user.value = null
    } finally {
      loading.value = false
    }
  }

  async function linkMinecraft() {
    const linked = await invoke<{ username: string } | null>('spectra_link_minecraft').catch(() => null)
    if (linked && user.value) user.value = { ...user.value, mcUsername: linked.username }
    return linked
  }

  async function login() {
    error.value = null
    const url = await invoke<string>('spectra_login_url')
    await invoke('open_external', { url })
  }

  async function logout() {
    await invoke('spectra_logout').catch(() => {})
    user.value = null
  }

  const displayName = computed(() => user.value?.username || user.value?.name || '')

  return { user, loading, error, isSignedIn, displayName, api, refresh, login, logout, linkMinecraft }
}

export function spectraInitial(name?: string | null) {
  const label = (name || '?').trim()
  let hue = 0
  for (const ch of label) hue = (hue * 31 + ch.charCodeAt(0)) % 360
  return { letter: label[0]!.toUpperCase(), hue }
}
