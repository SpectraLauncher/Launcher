import { open } from '@tauri-apps/plugin-shell'

export default defineNuxtPlugin(() => {
  if (import.meta.client) {
    document.addEventListener('click', (e) => {
      const target = e.target as HTMLElement
      const anchor = target.closest('a')

      if (anchor && anchor.href) {
        // Sprawdzamy czy link jest zewnętrzny (http/https) i nie prowadzi do lokalnej domeny aplikacji (tauri://, http://localhost itp)
        if (
          (anchor.href.startsWith('http://') || anchor.href.startsWith('https://')) &&
          !anchor.href.startsWith(window.location.origin)
        ) {
          e.preventDefault()
          open(anchor.href).catch(() => {})
        }
      }
    })
  }
})
