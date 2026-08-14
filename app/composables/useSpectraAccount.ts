import { invoke } from '@tauri-apps/api/core'

/** The signed-in Spectra user, as the website reports it. */
export interface SpectraUser {
  id: string
  name: string | null
  username: string | null
  image: string | null
  email?: string
  /** The Minecraft profile linked to this account, once verified. */
  mcUsername?: string | null
  mcUuid?: string | null
}

export interface SpectraFriend {
  id: string
  name: string | null
  username: string | null
  image: string | null
  mcUsername?: string | null
  friendshipId: number
}

export interface FriendRequest {
  id: number
  created: number
  user: Omit<SpectraFriend, 'friendshipId'>
}

/**
 * The launcher's Spectra account (friends, shared instances) — not to be
 * confused with `useAccountStore`, which holds Minecraft/Microsoft accounts.
 *
 * Every request goes through the Rust side, which owns the session token; the
 * frontend never sees it.
 */
export const useSpectraAccount = () => {
  const user = useState<SpectraUser | null>('spectra-user', () => null)
  const loading = useState('spectra-loading', () => false)
  const error = useState<string | null>('spectra-error', () => null)

  const isSignedIn = computed(() => user.value != null)

  /** Calls the Spectra API with the stored session token attached. */
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

  /**
   * Tells the site which Minecraft profile this account plays as, so friends can
   * be found by their in-game name. The token never passes through here — Rust
   * reads it from the account file and posts it straight to the site.
   */
  async function linkMinecraft() {
    const linked = await invoke<{ username: string } | null>('spectra_link_minecraft').catch(() => null)
    if (linked && user.value) user.value = { ...user.value, mcUsername: linked.username }
    return linked
  }

  /** Opens the browser; the session arrives back through the deep link. */
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

/** Colour + letter for someone without a picture, stable per name. */
export function spectraInitial(name?: string | null) {
  const label = (name || '?').trim()
  let hue = 0
  for (const ch of label) hue = (hue * 31 + ch.charCodeAt(0)) % 360
  return { letter: label[0]!.toUpperCase(), hue }
}
