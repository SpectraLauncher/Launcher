export default defineNuxtPlugin(() => {
  const theme = useThemeStore()
  theme.apply()
})
