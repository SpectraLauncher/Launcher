import { invoke } from '@tauri-apps/api/core'
import { toValue, type MaybeRefOrGetter } from 'vue'
import type { QuickPlay } from '~/types/launcher'

export const useMinecraftLaunch = (instanceId?: MaybeRefOrGetter<string | undefined>) => {
  const ac = useActivityCenter()
  const instances = useInstancesStore()
  const telemetry = useTelemetry()

  const launchingIds = useState<Record<string, boolean>>('mc-launching-ids', () => ({}))
  const errors = useState<Record<string, string | null>>('mc-errors', () => ({}))

  const id = computed(() => toValue(instanceId))

  const activity = computed(() => (id.value ? ac.activityFor(id.value).value : ac.top.value))

  const stage = computed<'idle' | 'installing' | 'running'>(() => {
    const a = activity.value
    if (!a) return 'idle'
    return a.kind === 'install' ? 'installing' : 'running'
  })

  const progress = computed(() => {
    const a = activity.value
    return a && a.kind === 'install'
      ? { current: a.current, total: a.total }
      : { current: 0, total: 0 }
  })

  const log = computed(() => (id.value ? ac.logsFor(id.value).value : []))
  const error = computed(() => (id.value ? errors.value[id.value] ?? null : null))
  const launching = computed(() => (id.value ? !!launchingIds.value[id.value] : Object.values(launchingIds.value).some(Boolean)))

  const runningId = computed(() => ac.list.value.find(a => a.kind === 'running')?.instanceId ?? null)

  const launch = async (launchId: string, quickPlay?: QuickPlay) => {
    errors.value = { ...errors.value, [launchId]: null }
    ac.clearLog(launchId)
    launchingIds.value = { ...launchingIds.value, [launchId]: true }
    const inst = instances.instances.find(i => i.id === launchId)
    if (inst) inst.last_played = new Date().toISOString()
    await ac.attach()
    try {
      await invoke('launch_instance', { id: launchId, quickPlay: quickPlay ?? null })
      telemetry.track('launch', { loader: inst?.loader.type, mc: inst?.mc_version })
      instances.load()
    } catch (e) {
      errors.value = { ...errors.value, [launchId]: String(e) }
      ac.clear(launchId)
      throw e
    } finally {
      launchingIds.value = { ...launchingIds.value, [launchId]: false }
    }
  }

  return {
    launching,
    runningId,
    stage,
    progress,
    log,
    error,
    launch,
    attach: ac.attach,
    detach: ac.detach,
  }
}
