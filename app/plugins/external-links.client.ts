import { openExternal } from '~/utils/openExternal'

export default defineNuxtPlugin(() => {
  if (import.meta.client) {
    document.addEventListener('click', (e) => {
      const target = e.target as HTMLElement
      const anchor = target.closest('a')

      if (anchor && anchor.href) {
        if (
          (anchor.href.startsWith('http://') || anchor.href.startsWith('https://')) &&
          !anchor.href.startsWith(window.location.origin)
        ) {
          e.preventDefault()
          openExternal(anchor.href).catch(() => {})
        }
      }
    })
  }
})
