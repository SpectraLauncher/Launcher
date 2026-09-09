import { marked } from 'marked'
import type { ModrinthGalleryItem } from '~/types/modrinth'

export function useProjectDetailView() {
  const bodyHtml = ref('')
  const loadingBody = ref(false)
  const detailTab = ref<'description' | 'gallery'>('description')
  const gallery = ref<ModrinthGalleryItem[]>([])
  const galleryIndex = ref<number | null>(null)

  const galleryImage = computed(() =>
    galleryIndex.value !== null ? gallery.value[galleryIndex.value] ?? null : null,
  )

  const galleryOpen = computed({
    get: () => galleryIndex.value !== null,
    set: (v: boolean) => { if (!v) galleryIndex.value = null },
  })

  function stepGallery(delta: number) {
    if (galleryIndex.value === null || !gallery.value.length) return
    const n = gallery.value.length
    galleryIndex.value = (galleryIndex.value + delta + n) % n
  }

  async function renderBody(md: string) {
    if (!md) {
      bodyHtml.value = ''
      return
    }
    const raw = marked.parse(md, { async: false }) as string
    const DOMPurify = (await import('dompurify')).default
    bodyHtml.value = DOMPurify.sanitize(raw, { ADD_ATTR: ['target'] })
  }

  function onGalleryKey(e: KeyboardEvent) {
    if (galleryIndex.value === null) return
    if (e.key === 'ArrowRight') { e.preventDefault(); stepGallery(1) }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); stepGallery(-1) }
    else if (e.key === 'Escape') galleryIndex.value = null
  }

  onMounted(() => window.addEventListener('keydown', onGalleryKey))
  onBeforeUnmount(() => window.removeEventListener('keydown', onGalleryKey))

  return {
    bodyHtml,
    loadingBody,
    detailTab,
    gallery,
    galleryIndex,
    galleryImage,
    galleryOpen,
    stepGallery,
    renderBody,
  }
}
