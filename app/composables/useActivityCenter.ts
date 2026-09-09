import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import type { MultiProgress, ExitInfo, CrashInfo, ModpackProgress } from '~/types/launcher'

interface ConsoleChunk { lines: string[], cursor: number, reset: boolean }

const CONSOLE_POLL_MS = 400
const CONSOLE_KEEP = 5000

export interface Activity {
  instanceId: string
  kind: 'install' | 'running'
  current: number
  total: number
}

const PRIORITY: Record<Activity['kind'], number> = {
  install: 2,
  running: 1,
}

let unlisteners: UnlistenFn[] = []
let attachPromise: Promise<void> | null = null
let consoleTimer: ReturnType<typeof setInterval> | null = null
const cursors = new Map<string, number>()

export const useActivityCenter = () => {
  const activities = useState<Record<string, Activity>>('mc-activities', () => ({}))
  const logs = useState<Record<string, string[]>>('mc-logs', () => ({}))
  const crashes = useState<Record<string, CrashInfo>>('mc-crashes', () => ({}))
  const modpack = useState<{ name: string, current: number, total: number } | null>('mc-modpack', () => null)
  const tasks = useState<Record<string, string>>('mc-tasks', () => ({}))

  function startTask(label: string): string {
    const id = crypto.randomUUID()
    tasks.value = { ...tasks.value, [id]: label }
    return id
  }
  function endTask(id: string) {
    if (!tasks.value[id]) return
    const next = { ...tasks.value }
    delete next[id]
    tasks.value = next
  }
  async function withTask<T>(label: string, fn: () => Promise<T>): Promise<T> {
    const id = startTask(label)
    try {
      return await fn()
    } finally {
      endTask(id)
    }
  }

  const upsert = (id: string, patch: Partial<Activity> & Pick<Activity, 'kind'>) => {
    const prev = activities.value[id]
    activities.value = {
      ...activities.value,
      [id]: { instanceId: id, current: 0, total: 0, ...prev, ...patch },
    }
  }

  const clear = (id: string) => {
    if (!activities.value[id]) return
    const next = { ...activities.value }
    delete next[id]
    activities.value = next
  }

  async function pumpConsole(id: string) {
    let chunk: ConsoleChunk
    try {
      chunk = await invoke<ConsoleChunk>('read_console', { id, cursor: cursors.get(id) ?? 0 })
    } catch {
      return
    }
    if (!chunk.lines.length && !chunk.reset) {
      cursors.set(id, chunk.cursor)
      return
    }
    const buf = chunk.reset ? [] : (logs.value[id] ?? []).slice()
    buf.push(...chunk.lines)
    if (buf.length > CONSOLE_KEEP) buf.splice(0, buf.length - CONSOLE_KEEP)
    cursors.set(id, chunk.cursor)
    logs.value = { ...logs.value, [id]: buf }
  }

  function pollConsoles() {
    const ids = Object.keys(activities.value)
    if (!ids.length) return
    for (const id of ids) void pumpConsole(id)
  }

  const attach = () => {
    if (!attachPromise) {
      consoleTimer ??= setInterval(pollConsoles, CONSOLE_POLL_MS)
      attachPromise = (async () => {
        unlisteners.push(
          await listen<MultiProgress>('mc://multi-progress', (e) => {
            upsert(e.payload.instance_id, { kind: 'install', current: e.payload.current, total: e.payload.total })
          }),
          await listen<ExitInfo>('mc://exited', (e) => {
            void pumpConsole(e.payload.instance_id)
            clear(e.payload.instance_id)
          }),
          await listen<CrashInfo>('mc://crashed', (e) => {
            const iid = e.payload.instance_id
            crashes.value = { ...crashes.value, [iid]: e.payload }
            crashInstance.value = iid
            crashOpen.value = true
          }),
          await listen<ModpackProgress>('modrinth://modpack-progress', (e) => {
            const { name, current, total } = e.payload
            modpack.value = { name, current, total }
            if (total > 0 && current >= total) {
              setTimeout(() => {
                if (modpack.value && modpack.value.current >= modpack.value.total) modpack.value = null
              }, 1500)
            }
          }),
        )
      })()
    }
    return attachPromise
  }

  const detach = () => {
    unlisteners.forEach(u => u())
    unlisteners = []
    attachPromise = null
    if (consoleTimer) {
      clearInterval(consoleTimer)
      consoleTimer = null
    }
  }

  const list = computed(() => Object.values(activities.value))

  const top = computed<Activity | null>(() => {
    let best: Activity | null = null
    for (const a of list.value) {
      if (!best || PRIORITY[a.kind] > PRIORITY[best.kind]) best = a
    }
    return best
  })

  const markRunning = (id: string) => {
    if (activities.value[id]) return
    upsert(id, { kind: 'running' })
  }

  const activityFor = (id: string) => computed(() => activities.value[id] ?? null)
  const logsFor = (id: string) => computed(() => logs.value[id] ?? [])
  const clearLog = (id: string) => {
    logs.value = { ...logs.value, [id]: [] }
    cursors.delete(id)
    void invoke('clear_console', { id }).catch(() => {})
  }

  const taskLabels = computed(() => Object.values(tasks.value))

  const liveLogsOpen = useState('mc-livelogs-open', () => false)
  const liveLogsInstance = useState<string | null>('mc-livelogs-instance', () => null)
  function openLiveLogs(instanceId?: string) {
    liveLogsInstance.value = instanceId ?? top.value?.instanceId ?? null
    liveLogsOpen.value = true
  }

  const crashOpen = useState('mc-crash-open', () => false)
  const crashInstance = useState<string | null>('mc-crash-instance', () => null)
  const crashFor = (id: string) => computed(() => crashes.value[id] ?? null)
  const clearCrash = (id: string) => {
    if (!crashes.value[id]) return
    const next = { ...crashes.value }
    delete next[id]
    crashes.value = next
  }

  return { activities, logs, tasks, taskLabels, startTask, endTask, withTask, attach, detach, list, top, activityFor, logsFor, clear, clearLog, markRunning, modpack, liveLogsOpen, liveLogsInstance, openLiveLogs, crashOpen, crashInstance, crashFor, clearCrash }
}
