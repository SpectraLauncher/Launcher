import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { platform, arch } from '@tauri-apps/plugin-os'
import type { Settings } from '~/types/launcher'

const TELEMETRY_ENDPOINT = 'https://spectra.makoto.com.pl/api/telemetry'

interface QueuedEvent {
  event: 'app_start' | 'launch' | 'feature' | 'update' | 'crash'
  props?: Record<string, unknown>
}

interface Meta {
  install_id: string
  version: string
  os: string
  arch: string
  locale: string
}

let enabled = false
let meta: Meta | null = null
let queue: QueuedEvent[] = []
let flushTimer: ReturnType<typeof setTimeout> | null = null
let initialized = false

function installId(): string {
  const KEY = 'spectra-install-id'
  let id = localStorage.getItem(KEY)
  if (!id) {
    id = crypto.randomUUID()
    localStorage.setItem(KEY, id)
  }
  return id
}

async function flush() {
  flushTimer = null
  if (!enabled || !meta || !queue.length) return
  const batch = queue
  queue = []
  try {
    await fetch(TELEMETRY_ENDPOINT, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ ...meta, events: batch }),
    })
  } catch {
  }
}

function scheduleFlush() {
  if (flushTimer) return
  flushTimer = setTimeout(flush, 4000)
}

export const useTelemetry = () => {
  function track(event: QueuedEvent['event'], props?: Record<string, unknown>) {
    if (!enabled) return
    queue.push({ event, props })
    scheduleFlush()
  }

  async function init() {
    if (initialized) return
    initialized = true
    try {
      const settings = await invoke<Settings>('get_settings')
      if (!settings.anonymous_stats) return
    } catch {
      return
    }
    try {
      meta = {
        install_id: installId(),
        version: await getVersion(),
        os: await platform(),
        arch: await arch(),
        locale: navigator.language || 'unknown',
      }
      enabled = true
      track('app_start')
    } catch {
      enabled = false
    }
  }

  function setEnabled(value: boolean) {
    enabled = value && meta !== null
    if (value && !initialized) init()
  }

  return { track, init, setEnabled }
}
