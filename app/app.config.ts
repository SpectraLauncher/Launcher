export default defineAppConfig({
  ui: {
    colors: {
      primary: 'sky',
      neutral: 'neutral',
    },

    card: {
      slots: {
        root: 'rounded-lg overflow-hidden',
        header: 'p-4 sm:px-6',
        title: 'text-highlighted font-semibold',
        description: 'mt-1 text-muted text-sm',
        body: 'p-4 sm:p-6',
        footer: 'p-4 sm:px-6'
      },
      variants: {
        variant: {
          solid: {
            root: 'bg-inverted text-inverted',
            title: 'text-inverted',
            description: 'text-dimmed'
          },
          outline: {
            root: 'bg-white/3 backdrop-blur-xs ring ring-default divide-y divide-default'
          },
          soft: {
            root: 'bg-white/3 backdrop-blur-xs divide-y divide-default'
          },
          subtle: {
            root: 'bg-white/3 backdrop-blur-xs ring-default divide-y divide-default'
          }
        }
      },
      defaultVariants: {
        variant: 'outline'
      }
    },

    modal: {
      slots: {
        overlay: 'fixed inset-0 backdrop-blur-sm'
      },
      variants: {
        overlay: {
          true: {
            overlay: 'bg-primary-500/10 '
          }
        },
      }
    }
  },

})
