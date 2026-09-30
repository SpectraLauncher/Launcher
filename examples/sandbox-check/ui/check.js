const rows = document.getElementById('rows')
const summary = document.getElementById('summary')

const rawCall = (method) => new Promise((resolve, reject) => {
  const id = 900000 + Math.floor(Math.random() * 90000)
  const onReply = (event) => {
    if (event.source !== parent || event.data?.spectra !== 1 || event.data.id !== id) return
    window.removeEventListener('message', onReply)
    if (event.data.error) reject(Object.assign(new Error(event.data.error.message), { code: event.data.error.code }))
    else resolve(event.data.result)
  }
  window.addEventListener('message', onReply)
  parent.postMessage({ spectra: 1, id, method, params: {} }, '*')
})

const fails = async (promise, code) => {
  try {
    await promise
    return 'it worked, and it should not have'
  } catch (error) {
    if (!code) return true
    return error.code === code || `failed with ${error.code}: ${error.message}`
  }
}

const tryInvoke = (command) => {
  const internals = window.__TAURI_INTERNALS__
  if (!internals?.invoke) return Promise.resolve('no bridge')
  return Promise.race([
    new Promise(resolve => resolve(internals.invoke(command, {}))).then(() => 'answered', () => 'refused'),
    new Promise(resolve => setTimeout(() => resolve('no answer'), 3000)),
  ])
}

const INFO = Symbol('info')

const CHECKS = [
  ['Launcher commands cannot be called from here', async () => {
    const outcomes = {
      get_launcher_paths: await tryInvoke('get_launcher_paths'),
      addons_list: await tryInvoke('addons_list'),
      'plugin:app|version': await tryInvoke('plugin:app|version'),
    }
    const answered = Object.entries(outcomes).filter(([, outcome]) => outcome === 'answered').map(([command]) => command)
    return answered.length === 0 || `answered: ${answered.join(', ')}`
  }],
  ['Tauri scripts in this frame', () => [INFO,
    typeof window.__TAURI_INTERNALS__ === 'undefined'
      ? 'absent'
      : 'present - WebView2 adds them to every frame; the row above shows they cannot be used']],
  ['Served from outside the launcher origin', () => location.hostname === '127.0.0.1' || `served from ${location.host}`],
  ['The frame has an opaque origin', () => window.origin === 'null' || `origin is ${window.origin}`],
  ['The launcher page is out of reach', () => {
    try {
      return parent.document ? 'parent.document is readable' : 'parent.document is empty'
    } catch {
      return true
    }
  }],
  ['Browser storage is off', () => {
    try {
      localStorage.getItem('probe')
      return 'localStorage works'
    } catch {
      return true
    }
  }],
  ['Popups are blocked', () => window.open('https://example.com') === null || 'a popup opened'],
  ['Direct network is blocked', () => fails(fetch('https://example.com/'))],
  ['The IPC endpoint is blocked', () => fails(fetch('http://ipc.localhost/plugin%3Aapp%7Cversion', { method: 'POST' }))],
  ['The addon can read its own files', async () => (await fetch('../addon.json')).ok || 'addon.json did not load'],
  ['Granted: instances.list', async () => Array.isArray(await spectra.instances.list()) || 'not a list'],
  ['Refused without permission: logs.list', () => fails(spectra.logs.list('any'), 'permission_denied')],
  ['Refused: a launcher command by name', () => fails(rawCall('auth_login'), 'unknown_method')],
  ['Allowed host: api.github.com', async () => {
    const response = await spectra.http.fetch('https://api.github.com/zen')
    return response.status === 200 || `status ${response.status}`
  }],
  ['Refused: a host not in the manifest', () => fails(spectra.http.fetch('https://example.com/'), 'permission_denied')],
  ['Refused: plain http', () => fails(spectra.http.fetch('http://api.github.com/zen'), 'permission_denied')],
  ['Storage keeps a value', async () => {
    const value = Date.now()
    await spectra.storage.set('probe', value)
    return (await spectra.storage.get('probe')) === value || 'the value changed'
  }],
  ['The launcher answers', async () => typeof (await spectra.launcher.version()) === 'string' || 'no version'],
]

const render = (results) => {
  rows.replaceChildren(...results.map(([name, state, detail]) => {
    const row = document.createElement('tr')
    const badge = document.createElement('span')
    badge.className = `badge ${state}`
    badge.textContent = state.toUpperCase()
    const cells = [badge, name, detail ?? ''].map((content, i) => {
      const cell = document.createElement('td')
      if (i === 2) cell.className = 'detail'
      cell.append(content)
      return cell
    })
    row.append(...cells)
    return row
  }))
  const checks = results.filter(r => r[1] !== 'info').length
  const failed = results.filter(r => r[1] === 'fail').length
  const waiting = results.filter(r => r[1] === 'wait').length
  summary.textContent = waiting
    ? 'Running…'
    : failed ? `${failed} of ${checks} checks failed` : `All ${checks} checks passed`
}

const run = async () => {
  const results = CHECKS.map(([name]) => [name, 'wait', ''])
  render(results)
  await Promise.all(CHECKS.map(async ([name, check], i) => {
    try {
      const outcome = await check()
      if (Array.isArray(outcome) && outcome[0] === INFO) results[i] = [name, 'info', outcome[1]]
      else results[i] = [name, outcome === true ? 'pass' : 'fail', outcome === true ? '' : String(outcome)]
    } catch (error) {
      results[i] = [name, 'fail', error.message]
    }
    render(results)
  }))
}

document.getElementById('again').addEventListener('click', run)
run()
