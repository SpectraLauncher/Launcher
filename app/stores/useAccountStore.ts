import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { Account, AccountsFile } from '~/types/launcher'

export const useAccountStore = defineStore('accounts', {
  state: () => ({
    accounts: [] as Account[],
    activeUuid: null as string | null,
    loading: false,
    error: null as string | null,
  }),
  getters: {
    activeAccount(state): Account | undefined {
      return state.accounts.find(a => a.uuid === state.activeUuid)
    },
    isLoggedIn(state): boolean {
      return state.activeUuid != null
    },
  },
  actions: {
    async load() {
      this.loading = true
      this.error = null
      try {
        const file = await invoke<AccountsFile>('list_accounts')
        this.accounts = file.accounts
        this.activeUuid = file.active_uuid ?? null
      } catch (e) {
        this.error = String(e)
      } finally {
        this.loading = false
      }
    },

    async login() {
      this.loading = true
      this.error = null
      try {
        const account = await invoke<Account>('auth_login')
        await this.load()
        return account
      } catch (e) {
        this.error = String(e)
        throw e
      } finally {
        this.loading = false
      }
    },

    async loginOffline(username: string) {
      this.loading = true
      this.error = null
      try {
        const account = await invoke<Account>('auth_login_offline', { username })
        await this.load()
        return account
      } catch (e) {
        this.error = String(e)
        throw e
      } finally {
        this.loading = false
      }
    },

    async setActive(uuid: string) {
      await invoke('set_active_account', { uuid })
      this.activeUuid = uuid
    },

    async remove(uuid: string) {
      await invoke('remove_account', { uuid })
      await this.load()
    },
  },
})
