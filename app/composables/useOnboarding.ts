const KEY = 'spectra-onboarded'

export const useOnboarding = () => {
  const open = useState('onboarding-open', () => false)

  const isDone = () => import.meta.client && localStorage.getItem(KEY) === '1'
  const start = () => { open.value = true }
  const finish = () => {
    open.value = false
    if (import.meta.client) localStorage.setItem(KEY, '1')
  }

  return { open, isDone, start, finish }
}
