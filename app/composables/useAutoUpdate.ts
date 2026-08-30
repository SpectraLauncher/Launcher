import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'uptodate' | 'error'

let pendingUpdate: Update | null = null

// Runs once per launcher start, however many components ask for it.
let startupRun: Promise<void> | null = null

// A launcher that will not open because the update server is slow is worse than
// a launcher one version behind, so the startup check gives up after this.
// The download itself is never cut short — someone on bad wifi still gets it.
const CHECK_TIMEOUT_MS = 10_000

export const useAutoUpdate = () => {
  const toast = useToast()
  const { t } = useI18n()

  const status = useState<UpdateStatus>('autoupdate:status', () => 'idle')
  const gate = useState<'checking' | 'updating' | 'done'>('autoupdate:gate', () => 'checking')
  const newVersion = useState<string>('autoupdate:version', () => '')
  const error = useState<string>('autoupdate:error', () => '')
  const downloaded = useState<number>('autoupdate:downloaded', () => 0)
  const total = useState<number>('autoupdate:total', () => 0)

  const available = computed(() => status.value === 'available' || status.value === 'downloading' || status.value === 'ready')
  const progress = computed(() => (total.value > 0 ? Math.min(100, Math.round((downloaded.value / total.value) * 100)) : 0))

  async function checkForUpdates(silent = true): Promise<boolean> {
    if (status.value === 'downloading') return true
    status.value = 'checking'
    error.value = ''
    try {
      const update = await check()
      if (update) {
        pendingUpdate = update
        newVersion.value = update.version
        status.value = 'available'
        return true
      }
      pendingUpdate = null
      status.value = silent ? 'idle' : 'uptodate'
      if (!silent) {
        toast.add({ title: t('update.uptodate'), icon: 'i-lucide-circle-check', color: 'success' })
      }
      return false
    } catch (e) {
      status.value = 'error'
      error.value = String(e)
      if (!silent) {
        toast.add({ title: t('update.error'), description: String(e), icon: 'i-lucide-alert-triangle', color: 'error' })
      }
      return false
    }
  }

  async function downloadAndInstall() {
    if (status.value === 'downloading') return
    if (!pendingUpdate) {
      const found = await checkForUpdates(false)
      if (!found || !pendingUpdate) return
    }
    const upd = pendingUpdate

    status.value = 'downloading'
    downloaded.value = 0
    total.value = 0
    error.value = ''
    try {
      await upd.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            total.value = event.data.contentLength ?? 0
            break
          case 'Progress':
            downloaded.value += event.data.chunkLength
            break
          case 'Finished':
            status.value = 'ready'
            break
        }
      })
      status.value = 'ready'
      await relaunch()
    } catch (e) {
      status.value = 'error'
      error.value = String(e)
      toast.add({ title: t('update.error'), description: String(e), icon: 'i-lucide-alert-triangle', color: 'error' })
    }
  }

  /**
   * Blocks the launcher on startup: look for an update first, and if there is
   * one, install it right away and restart into it.
   *
   * Every failure path falls through to a normal boot. Nobody is kept out of
   * their launcher because an update could not be checked, downloaded or
   * installed — worst case they carry on with the version they have.
   */
  function updateOnStartup(): Promise<void> {
    startupRun ??= (async () => {
      try {
        const found = await Promise.race([
          checkForUpdates(true),
          new Promise<false>(resolve => setTimeout(() => resolve(false), CHECK_TIMEOUT_MS)),
        ])
        if (!found || !pendingUpdate) return

        gate.value = 'updating'
        // On the happy path this relaunches, so nothing below runs.
        await downloadAndInstall()
      }
      catch {
        // Boot anyway; downloadAndInstall has already surfaced the reason.
      }
      finally {
        gate.value = 'done'
      }
    })()
    return startupRun
  }

  return {
    status,
    gate,
    updateOnStartup,
    newVersion,
    error,
    downloaded,
    total,
    available,
    progress,
    checkForUpdates,
    downloadAndInstall,
  }
}
