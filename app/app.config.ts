export default defineAppConfig({
  ui: {
    colors: {
      primary: 'sky',
      neutral: 'neutral',
    },

    button: {
      slots: {
        base: 'rounded-lg font-medium',
      },
    },

    card: {
      slots: {
        root: 'rounded-xl overflow-hidden',
        header: 'p-4 sm:px-5',
        title: 'text-highlighted font-semibold',
        description: 'mt-1 text-muted text-sm',
        body: 'p-4 sm:p-5',
        footer: 'p-4 sm:px-5'
      },
      variants: {
        variant: {
          solid: {
            root: 'bg-inverted text-inverted',
            title: 'text-inverted',
            description: 'text-dimmed'
          },
          outline: {
            root: 'bg-muted ring ring-default divide-y divide-default'
          },
          soft: {
            root: 'bg-elevated divide-y divide-default'
          },
          subtle: {
            root: 'bg-elevated ring ring-default divide-y divide-default'
          }
        }
      },
      defaultVariants: {
        variant: 'outline'
      }
    },

    modal: {
      slots: {
        overlay: 'fixed inset-0 bg-black/60',
        content: 'bg-elevated ring-default rounded-xl shadow-[0_24px_64px_rgba(0,0,0,0.5)]',
      },
    },

    tooltip: {
      slots: {
        content: 'bg-elevated ring-default text-highlighted',
      },
    },
  },
})
