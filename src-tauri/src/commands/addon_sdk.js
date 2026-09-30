(() => {
  if (window.spectra) return

  const pending = new Map()
  const listeners = new Map()
  const commands = new Map()
  let next = 0

  const call = (method, params) => new Promise((resolve, reject) => {
    const id = ++next
    pending.set(id, { resolve, reject })
    parent.postMessage({ spectra: 1, id, method, params: params ?? {} }, '*')
  })

  window.addEventListener('message', (event) => {
    if (event.source !== parent) return
    const data = event.data
    if (!data || data.spectra !== 1) return

    if (typeof data.event === 'string') {
      for (const fn of listeners.get(data.event) ?? []) {
        try {
          fn(data.payload)
        } catch (error) {
          console.error(error)
        }
      }
      return
    }

    if (typeof data.command === 'string') {
      const fn = commands.get(data.command)
      if (fn) Promise.resolve().then(() => fn(data.context ?? {})).catch(error => console.error(error))
      return
    }

    const waiting = pending.get(data.id)
    if (!waiting) return
    pending.delete(data.id)
    if (data.error) {
      const error = new Error(data.error.message)
      error.code = data.error.code
      waiting.reject(error)
    } else {
      waiting.resolve(data.result)
    }
  })

  const on = (name, fn) => {
    if (!listeners.has(name)) {
      listeners.set(name, new Set())
      call('events.subscribe', { name }).catch(error => console.error(error))
    }
    listeners.get(name).add(fn)
    return () => listeners.get(name)?.delete(fn)
  }

  const register = (name, fn) => {
    commands.set(name, fn)
    call('commands.register', { name }).catch(error => console.error(error))
  }

  window.spectra = Object.freeze({
    api: 1,
    context: Object.freeze(Object.fromEntries(new URLSearchParams(location.search))),
    instances: Object.freeze({
      list: () => call('instances.list'),
      get: id => call('instances.get', { id }),
      content: (id, kind = 'mod') => call('instances.content', { id, kind }),
      worlds: id => call('instances.worlds', { id }),
      isRunning: id => call('instances.isRunning', { id }),
      update: (id, patch) => call('instances.update', { ...patch, id }),
      setContentEnabled: (id, kind, filename, enabled) => call('instances.setContentEnabled', { id, kind, filename, enabled }),
      launch: id => call('instances.launch', { id }),
      stop: id => call('instances.stop', { id }),
    }),
    logs: Object.freeze({
      console: (id, cursor = 0) => call('logs.console', { id, cursor }),
      list: id => call('logs.list', { id }),
      read: (id, rel) => call('logs.read', { id, rel }),
    }),
    servers: Object.freeze({
      ping: (host, port) => call('servers.ping', { host, port }),
    }),
    account: Object.freeze({
      minecraft: () => call('account.minecraft'),
      spectra: () => call('account.spectra'),
    }),
    skins: Object.freeze({
      list: () => call('skins.list'),
    }),
    files: Object.freeze({
      list: (id, path) => call('files.list', { id, path }),
      read: (id, path, options = {}) => call('files.read', { id, path, encoding: options.encoding }),
      write: (id, path, data, options = {}) => call('files.write', { id, path, data, encoding: options.encoding }),
      remove: (id, path) => call('files.remove', { id, path }),
      mkdir: (id, path) => call('files.mkdir', { id, path }),
    }),
    resourcepacks: Object.freeze({
      files: (id, filename) => call('resourcepacks.files', { id, filename }),
      read: (id, filename, path) => call('resourcepacks.read', { id, filename, path }),
      save: (id, filename, excluded) => call('resourcepacks.save', { id, filename, excluded }),
    }),
    http: Object.freeze({
      fetch: (url, init = {}) => call('http.fetch', { url, method: init.method, headers: init.headers, body: init.body }),
    }),
    storage: Object.freeze({
      get: key => call('storage.get', { key }),
      set: (key, value) => call('storage.set', { key, value }),
      remove: key => call('storage.remove', { key }),
      keys: () => call('storage.keys'),
    }),
    ui: Object.freeze({
      toast: (title, options = {}) => call('ui.toast', { title, description: options.description, color: options.color }),
      navigate: page => call('ui.navigate', { page }),
      openWindow: window => call('ui.openWindow', { window }),
      openUrl: url => call('ui.openUrl', { url }),
      confirm: message => call('ui.confirm', { message }),
    }),
    launcher: Object.freeze({
      version: () => call('launcher.version'),
      locale: () => call('launcher.locale'),
      theme: () => call('launcher.theme'),
    }),
    backend: Object.freeze({
      call: (fn, input) => call('backend.call', { fn, input }),
    }),
    events: Object.freeze({ on }),
    commands: Object.freeze({ register }),
  })
})()
