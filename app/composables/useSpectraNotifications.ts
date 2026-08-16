/**
 * Friend requests and instance invites, polled from the website.
 *
 * Polling rather than a socket: the launcher is a desktop app that is open for
 * hours, and one indexed query every half minute costs less than keeping a
 * connection alive through sleep, VPN switches and flaky Wi-Fi.
 */

export type NotificationKind = 'friend_request' | 'friend_accepted' | 'instance_invite' | 'instance_update'

export interface SpectraNotification {
  id: number
  kind: NotificationKind
  shareCode: string | null
  data: { name?: string, revision?: number } | null
  read: boolean
  created: number
  actor: { id: string, name: string | null, username: string | null, image: string | null } | null
}

const POLL_MS = 30_000

export const useSpectraNotifications = () => {
  const account = useSpectraAccount()
  const items = useState<SpectraNotification[]>('spectra-notifications', () => [])
  const unread = useState('spectra-unread', () => 0)

  // Module-level would survive HMR badly; a state key keeps one timer per app.
  const timer = useState<ReturnType<typeof setInterval> | null>('spectra-poll', () => null)

  async function poll() {
    if (!account.isSignedIn.value) return
    try {
      const res = await account.api<{ unread: number, notifications: SpectraNotification[] }>(
        'GET', '/api/notifications',
      )
      items.value = res.notifications
      unread.value = res.unread
    } catch {
      // Offline or signed out — the next tick tries again.
    }
  }

  function start() {
    if (timer.value) return
    poll()
    timer.value = setInterval(poll, POLL_MS)
  }

  function stop() {
    if (!timer.value) return
    clearInterval(timer.value)
    timer.value = null
  }

  /** Marks everything (or the given ids) read, and drops the badge. */
  async function markRead(ids?: number[]) {
    const target = ids ?? items.value.filter(n => !n.read).map(n => n.id)
    if (!target.length) return
    items.value = items.value.map(n => (target.includes(n.id) ? { ...n, read: true } : n))
    unread.value = Math.max(0, unread.value - target.length)
    await account.api('POST', '/api/notifications/read', { ids: target }).catch(() => {})
  }

  /** Drops a notification from the list once it has been acted on. */
  function dismiss(id: number) {
    const gone = items.value.find(n => n.id === id)
    items.value = items.value.filter(n => n.id !== id)
    if (gone && !gone.read) unread.value = Math.max(0, unread.value - 1)
  }

  /**
   * Throws one away for good. Local-only removal would not last: the next poll
   * fetches it again, because the server is what decides the list.
   */
  async function remove(id: number) {
    dismiss(id)
    await account.api('DELETE', `/api/notifications/${id}`).catch(() => {})
  }

  return { items, unread, poll, start, stop, markRead, dismiss, remove }
}
