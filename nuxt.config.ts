// https://nuxt.com/docs/api/configuration/nuxt-config
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

  // Icons ship inside the app instead of being fetched from api.iconify.design
  // at runtime. A launcher is a desktop app served from a custom scheme — on
  // macOS that request does not come back, which is why the "+" and friends
  // were missing there while Windows was fine. Bundled, there is nothing to
  // fetch: the icons also work with no internet at all.
  icon: {
    clientBundle: {
      // Everything referenced in the source, plus anything Nuxt UI adds itself.
      scan: true,
      includeCustomCollections: true,
      // Enough headroom for the ~90 in use; the build fails loudly if exceeded.
      sizeLimitKb: 512,
    },
    // No silent fall back to the network — a missing icon should be caught here,
    // not on somebody's machine.
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