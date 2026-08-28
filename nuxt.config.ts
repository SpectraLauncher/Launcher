export default defineNuxtConfig({
  compatibilityDate: '2025-07-15',
  devtools: { enabled: true },
  ssr: false,
  app: {
    head: {
    }
  },

  vite: {
    server: {
      strictPort: true,
       watch: {
        ignored: ['**/src-tauri/**']
      },
      hmr: {
        protocol: 'ws',
        host: '0.0.0.0',
        port: 3001,
      },
    },
    envPrefix: ['VITE_', 'TAURI_'],
    optimizeDeps: {
      include: ['vue-draggable-plus'],
    },
  },

  nitro: {
    static: true,
    ignore: ['src-tauri/**']
  },

  icon: {
    clientBundle: {
      scan: true,
      includeCustomCollections: true,
      sizeLimitKb: 512,
    },
    fallbackToApi: false,
  },

  css: ['~/assets/css/main.css'],

  modules: [
    '@pinia/nuxt',
    '@nuxt/scripts',
    '@nuxt/ui',
    '@nuxtjs/i18n',
  ],

  i18n: {
    strategy: 'no_prefix',
    defaultLocale: 'en',
    lazy: true,
    locales: [
      { code: 'en', name: 'English', file: 'en.json' },
      { code: 'pl', name: 'Polski', file: 'pl.json' },
      { code: 'de', name: 'Deutsch', file: 'de.json' },
      { code: 'es', name: 'Español', file: 'es.json' },
      { code: 'fr', name: 'Français', file: 'fr.json' },
      { code: 'zh', name: '中文', file: 'zh.json' },
      { code: 'ru', name: 'Русский', file: 'ru.json' },
    ],

    detectBrowserLanguage: {
      useCookie: true,
      cookieKey: 'spectra_locale',
      fallbackLocale: 'en',
      alwaysRedirect: false,
    },
  },
})
