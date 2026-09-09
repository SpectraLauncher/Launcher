import { ref, toValue, watch, type MaybeRefOrGetter } from 'vue'

export function useThumbnail(
  path: MaybeRefOrGetter<string | null | undefined>,
  size: MaybeRefOrGetter<number>,
) {
  const src = ref<string>('')

  watch(
    [() => toValue(path), () => toValue(size)],
    ([source, pixels], _prev, onCleanup) => {
      if (!source) {
        src.value = ''
        return
      }
      let active = true
      onCleanup(() => { active = false })
      thumbnailUrl(source, pixels).then((url) => {
        if (active) src.value = url
      })
    },
    { immediate: true },
  )

  return src
}
