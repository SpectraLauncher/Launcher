<template>
  <svg
    viewBox="0 0 30 20"
    preserveAspectRatio="xMidYMid slice"
    class="overflow-hidden rounded-[3px] ring-1 ring-black/25"
    role="img"
    :aria-label="label"
  >
    <template v-if="code === 'pl'">
      <rect width="30" height="10" fill="#fff" />
      <rect y="10" width="30" height="10" fill="#dc143c" />
    </template>
    <template v-else-if="code === 'de'">
      <rect width="30" height="7" fill="#000" />
      <rect y="6.67" width="30" height="6.67" fill="#dd0000" />
      <rect y="13.33" width="30" height="6.67" fill="#ffce00" />
    </template>
    <template v-else-if="code === 'fr'">
      <rect width="10" height="20" fill="#0055a4" />
      <rect x="10" width="10" height="20" fill="#fff" />
      <rect x="20" width="10" height="20" fill="#ef4135" />
    </template>
    <template v-else-if="code === 'ru'">
      <rect width="30" height="7" fill="#fff" />
      <rect y="6.67" width="30" height="6.67" fill="#0039a6" />
      <rect y="13.33" width="30" height="6.67" fill="#d52b1e" />
    </template>
    <template v-else-if="code === 'es'">
      <rect width="30" height="20" fill="#aa151b" />
      <rect y="5" width="30" height="10" fill="#f1bf00" />
    </template>
    <template v-else-if="code === 'zh'">
      <rect width="30" height="20" fill="#ee1c25" />
      <polygon v-for="(s, i) in chinaStars" :key="i" :points="s" fill="#ffde00" />
    </template>
    <template v-else>
      <!-- Union flag, 2:1 artwork centred and cropped to the 3:2 frame. -->
      <svg x="-5" width="40" height="20" viewBox="0 0 60 30">
        <clipPath :id="`${uid}-t`">
          <path d="M30,15h30v15zv15h-30zh-30v-15zv-15h30z" />
        </clipPath>
        <rect width="60" height="30" fill="#012169" />
        <path d="M0,0L60,30M60,0L0,30" stroke="#fff" stroke-width="6" />
        <path d="M0,0L60,30M60,0L0,30" :clip-path="`url(#${uid}-t)`" stroke="#c8102e" stroke-width="4" />
        <path d="M30,0v30M0,15h60" stroke="#fff" stroke-width="10" />
        <path d="M30,0v30M0,15h60" stroke="#c8102e" stroke-width="6" />
      </svg>
    </template>
  </svg>
</template>

<script setup lang="ts">
defineProps<{ code: string, label?: string }>()

const uid = useId()

// Five-pointed star as polygon points, rotated so one point faces (tx, ty).
function star(cx: number, cy: number, r: number, tx = cx, ty = cy - 1) {
  const start = Math.atan2(ty - cy, tx - cx)
  return Array.from({ length: 10 }, (_, i) => {
    const a = start + (i * Math.PI) / 5
    const rr = i % 2 ? r * 0.382 : r
    return `${(cx + rr * Math.cos(a)).toFixed(2)},${(cy + rr * Math.sin(a)).toFixed(2)}`
  }).join(' ')
}

// Layout of the PRC flag on a 30x20 field: one large star, four small ones facing it.
const chinaStars = [
  star(5, 5, 3),
  star(10, 2, 1, 5, 5),
  star(12, 4, 1, 5, 5),
  star(12, 7, 1, 5, 5),
  star(10, 9, 1, 5, 5),
]
</script>
