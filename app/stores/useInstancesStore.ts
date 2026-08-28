import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { Instance, Loader } from '~/types/launcher'

export const useInstancesStore = defineStore('instances', {
  state: () => ({
    instances: [] as Instance[],
    selectedId: null as string | null,
    loading: false,
    loaded: false,
    error: null as string | null,
  }),
  getters: {
    selected(state): Instance | undefined {
      return state.instances.find(i => i.id === state.selectedId)
    },
  },
  actions: {
    async load() {
      this.loading = true
      this.error = null
      try {
        this.instances = await invoke<Instance[]>('list_instances')
        this.loaded = true
        if (!this.instances.some(i => i.id === this.selectedId)) {
          this.selectedId = this.instances[0]?.id ?? null
        }
      } catch (e) {
        this.error = String(e)
      } finally {
        this.loading = false
      }
    },

    async ensureLoaded() {
      if (!this.loaded) await this.load()
    },

    select(id: string) {
      this.selectedId = id
    },

    async create(opts: {
      name: string
      mcVersion: string
      loader: Loader
      memoryMb?: number
      iconSourcePath?: string | null
    }) {
      const instance = await invoke<Instance>('create_instance', {
        name: opts.name,
        mcVersion: opts.mcVersion,
        loader: opts.loader,
        memoryMb: opts.memoryMb ?? null,
        iconSourcePath: opts.iconSourcePath ?? null,
      })
      this.instances.unshift(instance)
      this.selectedId = instance.id
      return instance
    },

    async update(instance: Instance) {
      await invoke('update_instance', { instance })
      const idx = this.instances.findIndex(i => i.id === instance.id)
      if (idx !== -1) this.instances[idx] = instance
    },

    async remove(id: string) {
      await invoke('delete_instance', { id })
      this.instances = this.instances.filter(i => i.id !== id)
      if (this.selectedId === id) this.selectedId = this.instances[0]?.id ?? null
    },
  },
})
