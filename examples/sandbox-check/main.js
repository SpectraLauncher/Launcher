const log = async (entry) => {
  const events = (await spectra.storage.get('events')) ?? []
  events.unshift({ ...entry, at: new Date().toISOString() })
  await spectra.storage.set('events', events.slice(0, 30))
}

for (const name of ['game:launch', 'game:exit', 'instance:created', 'instance:removed']) {
  spectra.events.on(name, async (payload) => {
    await log({ name, ...payload })
    if (name === 'game:exit' && (await spectra.storage.get('toastOnExit')) !== false) {
      await spectra.ui.toast('Sandbox Check saw the game close', { color: 'info' })
    }
  })
}

spectra.commands.register('count-mods', async ({ instanceId }) => {
  const mods = await spectra.instances.content(instanceId, 'mod')
  const enabled = mods.filter(mod => mod.enabled).length
  await spectra.ui.toast(`${enabled} of ${mods.length} mods are on`, { color: 'success' })
  await log({ name: 'command:count-mods', instanceId })
})

log({ name: 'main:started' })
