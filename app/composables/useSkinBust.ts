// eslint-disable-next-line @typescript-eslint/no-explicit-any
let viewerPromise: Promise<any> | null = null
let queue: Promise<unknown> = Promise.resolve()

async function getBustViewer() {
  if (!viewerPromise) {
    viewerPromise = (async () => {
      const { SkinViewer } = await import('skinview3d')
      const canvas = document.createElement('canvas')
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const v: any = new SkinViewer({ canvas, width: 160, height: 160, renderPaused: true })
      v.playerObject.skin.leftLeg.visible = false
      v.playerObject.skin.rightLeg.visible = false
      v.playerObject.cape.visible = false
      v.playerObject.elytra.visible = false
      v.playerObject.rotation.y = -0.4
      return v
    })()
  }
  return viewerPromise
}

export function useSkinBust() {
  function render(source: string, model: 'classic' | 'slim'): Promise<string> {
    const task = queue.then(async () => {
      const v = await getBustViewer()
      await v.loadSkin(source, { model: model === 'slim' ? 'slim' : 'default' })
      const cam = v.camera
      cam.fov = 45
      const centerY = 6
      cam.position.set(0, centerY, 29)
      cam.lookAt(0, centerY, 0)
      cam.updateProjectionMatrix()
      v.render()
      return v.canvas.toDataURL('image/png') as string
    })
    queue = task.catch(() => {})
    return task
  }

  return { render }
}
