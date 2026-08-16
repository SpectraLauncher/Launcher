<script setup lang="ts">
// Right-hand panel: Spectra account, friends and the invites that arrive from
// them. Collapsed it is gone entirely — no rail, no sliver, no reserved width.

import { invoke } from '@tauri-apps/api/core'
import type { SpectraFriend, FriendRequest } from '~/composables/useSpectraAccount'
import type { SpectraNotification } from '~/composables/useSpectraNotifications'

const { t } = useI18n()
const panel = useAccountPanel()
const account = useSpectraAccount()
const notifications = useSpectraNotifications()
const createModal = useCreateInstanceModal()
const instances = useInstancesStore()
const activity = useActivityCenter()

const friends = ref<SpectraFriend[]>([])
const incoming = ref<FriendRequest[]>([])
const outgoing = ref<FriendRequest[]>([])
const query = ref('')
const busy = ref('')
const error = ref('')
const notice = ref('')

async function run(key: string, fn: () => Promise<unknown>) {
  busy.value = key
  error.value = ''
  notice.value = ''
  try {
    await fn()
  } catch (e) {
    error.value = String(e)
  } finally {
    busy.value = ''
  }
}

async function loadFriends() {
  if (!account.isSignedIn.value) return
  try {
    const res = await account.api<{ friends: SpectraFriend[], incoming: FriendRequest[], outgoing: FriendRequest[] }>(
      'GET', '/api/friends',
    )
    friends.value = res.friends
    incoming.value = res.incoming
    outgoing.value = res.outgoing
  } catch (e) {
    error.value = String(e)
  }
}

// --- who am I actually inviting? -------------------------------------------
// A Minecraft name and a Spectra name are often different, so typing one and
// hoping is how a request ends up with a stranger. These are the people it
// could be, with faces and both names, and you pick one.
type Candidate = SpectraFriend & { relation: 'friend' | 'pending' | null }
const results = ref<Candidate[]>([])
const searching = ref(false)
let searchTimer: ReturnType<typeof setTimeout> | null = null

watch(query, (value) => {
  if (searchTimer) clearTimeout(searchTimer)
  const q = value.trim()
  if (q.length < 2) {
    results.value = []
    return
  }
  // Debounced: this would fire on every keystroke otherwise.
  searchTimer = setTimeout(async () => {
    searching.value = true
    try {
      const res = await account.api<{ users: Candidate[] }>('GET', `/api/users?q=${encodeURIComponent(q)}`)
      results.value = res.users
    } catch {
      results.value = []
    } finally {
      searching.value = false
    }
  }, 250)
})

const invite = (candidate: Candidate) => run(`add-${candidate.id}`, async () => {
  // By id, not by name — whoever is on that row is who gets the request.
  await account.api('POST', '/api/friends', { userId: candidate.id })
  query.value = ''
  results.value = []
  notice.value = t('spectra.requestSent')
  await loadFriends()
})

/**
 * Enter sends. If the list narrowed to one person, that is who it means;
 * otherwise the text is taken as an exact name or e-mail address — the server
 * matches all three, and says so when it matches nothing.
 */
const addFriend = () => run('add', async () => {
  const q = query.value.trim()
  if (!q) return
  if (results.value.length === 1) return invite(results.value[0]!)

  await account.api('POST', '/api/friends', { query: q })
  query.value = ''
  results.value = []
  notice.value = t('spectra.requestSent')
  await loadFriends()
})

const answer = (id: number, action: 'accept' | 'reject') => run(`req-${id}`, async () => {
  await account.api('PATCH', `/api/friends/${id}`, { action })
  // The server drops the matching notification, so a re-poll is what makes it
  // disappear here too.
  await Promise.all([loadFriends(), notifications.poll()])
})

const removeFriend = (friendshipId: number) => run(`rm-${friendshipId}`, async () => {
  await account.api('DELETE', `/api/friends/${friendshipId}`)
  await loadFriends()
})

/** Same endpoint — either side of a friendship row may drop it. */
const cancelRequest = (id: number) => run(`rm-${id}`, async () => {
  await account.api('DELETE', `/api/friends/${id}`)
  // The candidate list should offer them again straight away.
  results.value = []
  await loadFriends()
})

/** An invite is just a share code — hand it to the importer we already have. */
function install(n: SpectraNotification) {
  if (!n.shareCode) return
  notifications.markRead([n.id])
  panel.close()
  createModal.openWithCode(n.shareCode)
}

/** The local instance that came from this code, if it is still around. */
const instanceFor = (code: string | null) =>
  code ? instances.instances.find(i => i.share_origin?.code === code) : undefined

/**
 * Applies the author's new revision to the copy already installed. Without a
 * local copy there is nothing to update, so fall back to a fresh install.
 */
const update = (n: SpectraNotification) => run(`upd-${n.id}`, async () => {
  const target = instanceFor(n.shareCode)
  if (!target) return install(n)

  await activity.withTask(t('spectra.updating', { name: target.name }), () =>
    invoke('sync_share', { id: target.id, code: n.shareCode }))
  await instances.load()
  // Downloading the pack is what clears it on the server; this just catches up.
  notifications.dismiss(n.id)
  await notifications.poll()
})

const label = (u: { username: string | null, name: string | null } | null) =>
  u?.username || u?.name || '—'

function notificationText(n: SpectraNotification) {
  const who = label(n.actor)
  const what = n.data?.name ?? ''
  return t(`spectra.note.${n.kind}`, { who, name: what })
}

// Loading friends only matters while the panel is on screen.
watch(() => panel.isOpen.value, (open) => {
  if (open && account.isSignedIn.value) {
    loadFriends()
    notifications.poll()
    notifications.markRead()
  }
})
watch(() => account.isSignedIn.value, signedIn => (signedIn ? loadFriends() : (friends.value = [])))
</script>

<template>
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="translate-x-full opacity-0"
    leave-active-class="transition duration-150 ease-in"
    leave-to-class="translate-x-full opacity-0"
  >
    <aside
      v-if="panel.isOpen.value"
      class="absolute top-0 right-0 z-20 flex h-full w-[330px] flex-col border-l border-white/8 bg-[#0b0e14]/95 backdrop-blur-xl"
    >
      <!-- header -->
      <div class="flex items-center gap-2 border-b border-white/8 px-4 py-3">
        <h2 class="flex-1 text-sm font-semibold text-neutral-200">{{ $t('spectra.title') }}</h2>
        <button
          type="button"
          class="rounded-lg p-1.5 text-neutral-500 transition hover:bg-white/5 hover:text-neutral-200 flex items-center justify-center"
          :title="$t('common.close')"
          @click="panel.close()"
        >
          <UIcon name="i-lucide-x" class="size-4" />
        </button>
      </div>

      <!-- signed out -->
      <div v-if="!account.isSignedIn.value" class="flex flex-1 flex-col items-center justify-center gap-4 px-6 text-center">
        <span class="flex size-14 items-center justify-center rounded-2xl border border-white/10 bg-primary-500/10 ">
          <UIcon name="i-lucide-users" class="size-6 text-primary-400" />
        </span>
        <div>
          <p class="text-sm font-semibold text-neutral-200">{{ $t('spectra.signedOutTitle') }}</p>
          <p class="mt-1.5 text-xs leading-relaxed text-neutral-500">{{ $t('spectra.signedOutHint') }}</p>
        </div>
        <button
          type="button"
          class="w-full rounded-lg bg-primary-500 py-2 text-sm font-semibold text-[#04121f] transition hover:bg-primary-400"
          @click="account.login()"
        >{{ $t('spectra.signIn') }}</button>
        <p class="text-[11px] text-neutral-600">{{ $t('spectra.browserHint') }}</p>
      </div>

      <template v-else>
        <!-- who -->
        <div class="flex items-center gap-3 border-b border-white/8 px-4 py-3">
          <img v-if="account.user.value?.image" :src="account.user.value.image" alt="" class="size-9 rounded-full object-cover">
          <span
            v-else
            class="flex size-9 items-center justify-center rounded-full text-sm font-bold text-white/90"
            :style="`background:hsl(${spectraInitial(account.displayName.value).hue} 55% 30%)`"
          >{{ spectraInitial(account.displayName.value).letter }}</span>
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-neutral-100">{{ account.displayName.value }}</p>
            <p class="truncate text-[11px] text-neutral-500">
              {{ account.user.value?.mcUsername || account.user.value?.email }}
            </p>
          </div>
          <button
            type="button"
            class="rounded-lg p-1.5 text-neutral-500 transition hover:bg-white/5 hover:text-neutral-200 flex items-center justify-center"
            :title="$t('spectra.signOut')"
            @click="account.logout()"
          >
            <UIcon name="i-lucide-log-out" class="size-4" />
          </button>
        </div>

        <div class="min-h-0 flex-1 space-y-5 overflow-y-auto px-4 py-4">
          <p v-if="error" class="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-xs text-red-300">
            {{ error }}
          </p>
          <p v-if="notice" class="rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-300">
            {{ notice }}
          </p>

          <!-- invites and updates -->
          <section v-if="notifications.items.value.length">
            <p class="text-[10px] font-semibold tracking-[0.14em] text-neutral-500 uppercase">
              {{ $t('spectra.inbox') }}
            </p>
            <div class="mt-2 space-y-2">
              <div
                v-for="n in notifications.items.value"
                :key="n.id"
                class="rounded-lg border border-white/8 bg-white/4 p-2.5"
              >
                <div class="flex items-start gap-2.5">
                  <img v-if="n.actor?.image" :src="n.actor.image" alt="" class="size-7 shrink-0 rounded-full object-cover">
                  <span
                    v-else
                    class="flex size-7 shrink-0 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
                    :style="`background:hsl(${spectraInitial(label(n.actor)).hue} 55% 30%)`"
                  >{{ spectraInitial(label(n.actor)).letter }}</span>
                  <p class="flex-1 text-xs leading-relaxed text-neutral-300">{{ notificationText(n) }}</p>
                  <button
                    type="button"
                    class="flex shrink-0 items-center justify-center rounded-md p-1 text-neutral-600 transition hover:bg-white/5 hover:text-neutral-200"
                    :title="$t('spectra.dismiss')"
                    @click="notifications.remove(n.id)"
                  >
                    <UIcon name="i-lucide-x" class="size-3.5" />
                  </button>
                </div>

                <div v-if="n.kind === 'instance_invite'" class="mt-2 flex justify-end">
                  <button
                    type="button"
                    class="rounded-md bg-primary-500 px-2.5 py-1 text-[11px] font-semibold text-[#04121f] transition hover:bg-primary-400"
                    @click="install(n)"
                  >{{ $t('spectra.install') }}</button>
                </div>

                <div v-else-if="n.kind === 'instance_update'" class="mt-2 flex justify-end">
                  <button
                    type="button"
                    class="rounded-md bg-primary-500 px-2.5 py-1 text-[11px] font-semibold text-[#04121f] transition hover:bg-primary-400 disabled:opacity-50"
                    :disabled="busy === `upd-${n.id}`"
                    @click="update(n)"
                  >{{ instanceFor(n.shareCode) ? $t('spectra.update') : $t('spectra.install') }}</button>
                </div>
              </div>
            </div>
          </section>

          <!-- friend requests -->
          <section v-if="incoming.length">
            <p class="text-[10px] font-semibold tracking-[0.14em] text-neutral-500 uppercase">
              {{ $t('spectra.requests') }}
            </p>
            <div class="mt-2 space-y-2">
              <div
                v-for="r in incoming"
                :key="r.id"
                class="flex items-center gap-2.5 rounded-lg border border-primary-500/20 bg-primary-500/8 px-2.5 py-2"
              >
                <span class="flex-1 truncate text-xs text-neutral-200">{{ label(r.user) }}</span>
                <button
                  type="button"
                  class="rounded-md bg-primary-500 px-2 py-1 text-[11px] font-semibold text-[#04121f] transition hover:bg-primary-400"
                  :disabled="busy === `req-${r.id}`"
                  @click="answer(r.id, 'accept')"
                >{{ $t('spectra.accept') }}</button>
                <button
                  type="button"
                  class="rounded-md px-2 py-1 text-[11px] text-neutral-400 transition hover:bg-white/5 hover:text-neutral-200"
                  :disabled="busy === `req-${r.id}`"
                  @click="answer(r.id, 'reject')"
                >{{ $t('spectra.reject') }}</button>
              </div>
            </div>
          </section>

          <!-- friends -->
          <section>
            <p class="text-[10px] font-semibold tracking-[0.14em] text-neutral-500 uppercase">
              {{ $t('spectra.friends') }} <span v-if="friends.length" class="text-neutral-600">· {{ friends.length }}</span>
            </p>

            <form class="mt-2 flex gap-1.5" @submit.prevent="addFriend">
              <input
                v-model="query"
                class="min-w-0 flex-1 rounded-lg border border-white/8 bg-white/4 px-2.5 py-1.5 text-xs text-neutral-100 outline-none transition placeholder:text-neutral-600 focus:border-primary-500/50"
                :placeholder="$t('spectra.addPlaceholder')"
              >
              <button
                type="submit"
                class="rounded-lg border border-white/8 bg-white/4 px-2.5 text-neutral-300 transition hover:border-primary-500/40 hover:text-neutral-100 disabled:opacity-40 flex items-center justify-center"
                :disabled="!query.trim() || busy === 'add'"
                :title="$t('spectra.add')"
              >
                <UIcon
                  :name="searching ? 'i-lucide-loader-circle' : 'i-lucide-user-plus'"
                  class="size-4"
                  :class="searching && 'animate-spin'"
                />
              </button>
            </form>

            <!-- who matches what is being typed -->
            <div v-if="query.trim().length >= 2" class="mt-1.5">
              <ul v-if="results.length" class="space-y-1">
                <li
                  v-for="c in results"
                  :key="c.id"
                  class="flex items-center gap-2.5 rounded-lg border border-white/8 bg-white/4 px-2.5 py-1.5"
                >
                  <img v-if="c.image" :src="c.image" alt="" class="size-7 shrink-0 rounded-full object-cover">
                  <span
                    v-else
                    class="flex size-7 shrink-0 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
                    :style="`background:hsl(${spectraInitial(label(c)).hue} 55% 30%)`"
                  >{{ spectraInitial(label(c)).letter }}</span>
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-xs text-neutral-200">{{ label(c) }}</span>
                    <span v-if="c.mcUsername" class="block truncate text-[10px] text-neutral-500">
                      {{ $t('spectra.inGame', { name: c.mcUsername }) }}
                    </span>
                  </span>
                  <span v-if="c.relation === 'friend'" class="shrink-0 text-[10px] text-neutral-500">
                    {{ $t('spectra.alreadyFriends') }}
                  </span>
                  <span v-else-if="c.relation === 'pending'" class="shrink-0 text-[10px] text-neutral-500">
                    {{ $t('spectra.pendingShort') }}
                  </span>
                  <button
                    v-else
                    type="button"
                    class="shrink-0 rounded-md bg-primary-500 px-2 py-1 text-[11px] font-semibold text-[#04121f] transition hover:bg-primary-400 disabled:opacity-50"
                    :disabled="busy === `add-${c.id}`"
                    @click="invite(c)"
                  >{{ $t('spectra.add') }}</button>
                </li>
              </ul>
              <p v-else-if="!searching" class="px-1 py-1.5 text-[11px] text-neutral-600">
                {{ $t('spectra.noMatches') }}
              </p>
            </div>

            <ul class="mt-2 space-y-1">
              <li
                v-for="f in friends"
                :key="f.id"
                class="group flex items-center gap-2.5 rounded-lg px-2 py-1.5 transition hover:bg-white/4"
              >
                <img v-if="f.image" :src="f.image" alt="" class="size-7 rounded-full object-cover">
                <span
                  v-else
                  class="flex size-7 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
                  :style="`background:hsl(${spectraInitial(label(f)).hue} 55% 30%)`"
                >{{ spectraInitial(label(f)).letter }}</span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-xs text-neutral-200">{{ label(f) }}</span>
                  <span v-if="f.mcUsername" class="block truncate text-[10px] text-neutral-500">
                    {{ f.mcUsername }}
                  </span>
                </span>
                <button
                  type="button"
                  class="rounded-md p-1 text-neutral-600 opacity-0 transition group-hover:opacity-100 hover:bg-white/5 hover:text-red-400 flex items-center justify-center"
                  :title="$t('spectra.removeFriend')"
                  @click="removeFriend(f.friendshipId)"
                >
                  <UIcon name="i-lucide-user-minus" class="size-3.5" />
                </button>
              </li>
            </ul>

            <p v-if="!friends.length" class="mt-3 text-center text-xs leading-relaxed text-neutral-600">
              {{ $t('spectra.noFriends') }}
            </p>

            <!-- requests you sent, and the way to take one back -->
            <div v-if="outgoing.length" class="mt-4">
              <p class="text-[10px] font-semibold tracking-[0.14em] text-neutral-500 uppercase">
                {{ $t('spectra.outgoing', { n: outgoing.length }) }}
              </p>
              <ul class="mt-2 space-y-1">
                <li
                  v-for="r in outgoing"
                  :key="r.id"
                  class="group flex items-center gap-2.5 rounded-lg bg-white/4 px-2.5 py-1.5"
                >
                  <img v-if="r.user.image" :src="r.user.image" alt="" class="size-7 shrink-0 rounded-full object-cover">
                  <span
                    v-else
                    class="flex size-7 shrink-0 items-center justify-center rounded-full text-[11px] font-bold text-white/90"
                    :style="`background:hsl(${spectraInitial(label(r.user)).hue} 55% 30%)`"
                  >{{ spectraInitial(label(r.user)).letter }}</span>
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-xs text-neutral-200">{{ label(r.user) }}</span>
                    <span v-if="r.user.mcUsername" class="block truncate text-[10px] text-neutral-500">
                      {{ $t('spectra.inGame', { name: r.user.mcUsername }) }}
                    </span>
                  </span>
                  <span class="shrink-0 text-[10px] text-neutral-600">{{ $t('spectra.pendingShort') }}</span>
                  <button
                    type="button"
                    class="flex shrink-0 items-center justify-center rounded-md p-1 text-neutral-600 transition hover:bg-white/5 hover:text-red-400"
                    :title="$t('spectra.cancelRequest')"
                    :disabled="busy === `rm-${r.id}`"
                    @click="cancelRequest(r.id)"
                  >
                    <UIcon name="i-lucide-x" class="size-3.5" />
                  </button>
                </li>
              </ul>
            </div>
          </section>
        </div>
      </template>
    </aside>
  </Transition>
</template>
