import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'uptodate' | 'error'

let pendingUpdate: Update | null = null

export const useAutoUpdate = () => {
  const toast = useToast()
  const { t } = useI18n()

  const status = useState<UpdateStatus>('autoupdate:status', () => 'idle')
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

  return {
    status,
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
