import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { ask } from '@tauri-apps/plugin-dialog'
import type { SlotButton } from '~/stores/useAddonsStore'

export interface AddonFrameHandle {
  addonId: string
  win: Window
  events: Set<string>
  commands: Set<string>
}

export interface AddonBridgeHost {
  toast: (toast: { title: string; description?: string; color: 'success' | 'error' | 'warning' | 'info' | 'neutral' }) => void
  navigate: (addonId: string, page: string) => void
  launch: (instanceId: string) => Promise<unknown>
  locale: () => string
  theme: () => { mode: string; accent: string }
  addonName: (addonId: string) => string
  openUrl: (url: string) => Promise<void>
  t: (key: string, params?: Record<string, unknown>) => string
}

const COLORS = ['success', 'error', 'warning', 'info', 'neutral'] as const

const frames = new Set<AddonFrameHandle>()
let host: AddonBridgeHost | null = null
let listening = false

function reply(frame: AddonFrameHandle, id: number, payload: Record<string, unknown>) {
  frame.win.postMessage({ spectra: 1, id, ...payload }, '*')
}

async function perform(frame: AddonFrameHandle, method: string, params: Record<string, unknown>) {
  if (method === 'events.subscribe') {
    frame.events.add(String(params.name ?? '').slice(0, 64))
    return null
  }
  if (method === 'commands.register') {
    frame.commands.add(String(params.name ?? '').slice(0, 64))
    return null
  }

  const answer = await invoke<unknown>('addon_call', { addon: frame.addonId, method, params })
  if (!answer || typeof answer !== 'object' || !('$host' in answer)) return answer
  if (!host) throw new Error('the launcher is not ready')

  const name = host.addonName(frame.addonId)
  switch (method) {
    case 'ui.toast': {
      const color = COLORS.find(c => c === params.color) ?? 'neutral'
      host.toast({
        title: String(params.title).slice(0, 200),
        description: typeof params.description === 'string' ? params.description.slice(0, 500) : name,
        color,
      })
      return null
    }
    case 'ui.navigate':
      host.navigate(frame.addonId, String(params.page))
      return null
    case 'ui.openUrl': {
      const url = String(params.url)
      const allowed = await ask(host.t('addons.openUrlAsk', { name, url }), { title: name, kind: 'warning' })
      if (allowed) await host.openUrl(url)
      return allowed
    }
    case 'ui.confirm':
      return await ask(String(params.message), { title: name })
    case 'launcher.locale':
      return host.locale()
    case 'launcher.theme':
      return host.theme()
    case 'instances.launch':
      await host.launch(String(params.id))
      return null
  }
  throw new Error(`${method} is not available here`)
}

function onMessage(event: MessageEvent) {
  const frame = [...frames].find(f => f.win === event.source)
  if (!frame) return
  const data = event.data as { spectra?: unknown; id?: unknown; method?: unknown; params?: unknown } | null
  if (!data || data.spectra !== 1 || typeof data.id !== 'number' || typeof data.method !== 'string') return
  const id = data.id

  let params: Record<string, unknown> = {}
  try {
    const copy = JSON.parse(JSON.stringify(data.params ?? {}))
    if (copy && typeof copy === 'object' && !Array.isArray(copy)) params = copy
  } catch {
    reply(frame, id, { error: { code: 'invalid', message: 'parameters have to be plain JSON' } })
    return
  }

  perform(frame, data.method.slice(0, 64), params)
    .then(result => reply(frame, id, { result: result ?? null }))
    .catch((e: unknown) => {
      const code = typeof (e as { code?: unknown })?.code === 'string' ? (e as { code: string }).code : 'error'
      reply(frame, id, { error: { code, message: errorDetail(e) } })
    })
}

export function initAddonBridge(next: AddonBridgeHost) {
  host = next
  if (!listening && import.meta.client) {
    window.addEventListener('message', onMessage)
    listening = true
  }
}

export function registerAddonFrame(addonId: string, win: Window): AddonFrameHandle {
  const frame = { addonId, win, events: new Set<string>(), commands: new Set<string>() }
  frames.add(frame)
  return frame
}

export function unregisterAddonFrame(frame: AddonFrameHandle) {
  frames.delete(frame)
}

export function emitAddonEvent(name: string, payload: Record<string, unknown>) {
  for (const frame of frames) {
    if (frame.events.has(name)) frame.win.postMessage({ spectra: 1, event: name, payload }, '*')
  }
}

export function broadcastAddonEvent(name: string, payload: Record<string, unknown>) {
  emit('addon://event', { name, payload }).catch(() => {})
}

export function sendAddonCommand(addonId: string, command: string, context: Record<string, string>): boolean {
  let delivered = false
  for (const frame of frames) {
    if (frame.addonId === addonId && frame.commands.has(command)) {
      frame.win.postMessage({ spectra: 1, command, context }, '*')
      delivered = true
    }
  }
  return delivered
}

export async function runAddonButton(button: SlotButton, context: Record<string, string> = {}): Promise<boolean> {
  const action = button.action
  if (action.type === 'url') await openExternal(action.url)
  else if (action.type === 'page') host?.navigate(button.addonId, action.page)
  else if (action.type === 'window') await invoke('addons_open_window', { addon: button.addonId, window: action.window })
  else return sendAddonCommand(button.addonId, action.command, context)
  return true
}
