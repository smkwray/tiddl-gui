<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { store } from "./lib/stores.svelte";
  import type {
    TiddlStatus,
    FfmpegStatus,
    AuthStatus,
    ConfigDto,
    QueueItem,
    QueueItemEvent,
    DownloadProgressEvent,
    DownloadLogEvent,
    DownloadFinishedEvent,
    AuthFlowEvent,
  } from "./lib/types";
  import Header from "./lib/Header.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import FirstRunGate from "./lib/FirstRunGate.svelte";
  import ActivityLog from "./lib/ActivityLog.svelte";
  import DownloadPanel from "./lib/DownloadPanel.svelte";
  import SearchPanel from "./lib/SearchPanel.svelte";
  import LibraryPanel from "./lib/LibraryPanel.svelte";
  import SettingsPanel from "./lib/SettingsPanel.svelte";
  import LoginModal from "./lib/LoginModal.svelte";
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";
  import HelpModal from "./lib/HelpModal.svelte";

  let unlisteners: UnlistenFn[] = [];
  let destroyed = false;
  type Confirm = { title: string; message: string; confirmLabel: string; danger: boolean; onconfirm: () => void };
  let confirm = $state<Confirm | null>(null);

  async function refreshAll() {
    store.tiddl = await invoke<TiddlStatus>("check_tiddl");
    if (store.tiddl.found) {
      store.ffmpeg = await invoke<FfmpegStatus>("check_ffmpeg");
      store.auth = await invoke<AuthStatus>("get_auth_status");
      store.config = await invoke<ConfigDto>("get_config");
      store.initOptions(store.config);
      store.setQueue(await invoke<QueueItem[]>("get_queue"));
    }
  }
  async function refreshAuth() {
    store.auth = await invoke<AuthStatus>("get_auth_status");
  }

  // The access token is short-lived (hours); the refresh token is long-lived.
  // So on most launches the token has expired but can be renewed silently — the
  // user shouldn't have to do anything. `tiddl auth login` is a no-op while any
  // token file exists ("Already logged in"), so only `auth_refresh` fixes this.
  // "Sign in" is reserved for a genuine login (no token, or a dead refresh token).
  async function autoRefreshIfExpired() {
    if (!store.auth || store.auth.authed || !store.auth.expired) return;
    store.refreshingAuth = true;
    try {
      const msg = await invoke<string>("auth_refresh");
      store.appendLog(`auto token refresh: ${msg}`);
    } catch (e) {
      store.appendLog(`auto token refresh failed: ${e}`);
    } finally {
      await refreshAuth();
      store.refreshingAuth = false;
    }
  }

  async function reg<T>(name: string, handler: (p: T) => void) {
    const un = await listen<T>(name, (e) => handler(e.payload));
    if (destroyed) un();
    else unlisteners.push(un);
  }

  onMount(async () => {
    store.initTheme();
    await refreshAll();
    await autoRefreshIfExpired();
    await reg<AuthFlowEvent>("auth-flow", (p) => {
      store.setAuthFlow(p);
      if (p.phase === "success" || p.phase === "already") {
        store.showLogin = false;
        refreshAuth();
      }
    });
    await reg<QueueItemEvent>("queue-item", (p) => store.upsertItem(p.item));
    await reg<DownloadProgressEvent>("download-progress", (p) => store.patchProgress(p));
    await reg<DownloadLogEvent>("download-log", (p) => store.appendLog(`#${p.id}  ${p.line}`));
    await reg<DownloadFinishedEvent>("download-finished", (p) => {
      store.finishItem(p);
      if (p.leftover_note) store.appendLog(`#${p.id}  ${p.leftover_note}`);
    });
    await reg<Record<string, never>>("queue-empty", () => {});
    await reg<Record<string, never>>("close-requested", () => {
      confirm = {
        title: "Quit tiddl-gui?",
        message: "A download is still running. Cancel it and quit?",
        confirmLabel: "Cancel & quit",
        danger: true,
        onconfirm: () => {
          confirm = null;
          invoke("quit");
        },
      };
    });
  });
  onDestroy(() => {
    destroyed = true;
    unlisteners.forEach((u) => u());
  });

  function signIn() {
    store.authFlow = null;
    store.showLogin = true;
  }
  function signOut() {
    confirm = {
      title: "Sign out?",
      message: "Remove the stored login token from tiddl on this machine.",
      confirmLabel: "Sign out",
      danger: true,
      onconfirm: async () => {
        confirm = null;
        try {
          await invoke("auth_logout");
        } catch (e) {
          store.appendLog(`sign out: ${e}`);
        }
        await refreshAuth();
      },
    };
  }
  async function refreshAuthToken() {
    try {
      const msg = await invoke<string>("auth_refresh");
      store.appendLog(`token refresh: ${msg}`);
    } catch (e) {
      store.appendLog(`token refresh failed: ${e}`);
    }
    await refreshAuth();
  }

  // Primary modifier is ⌘ on macOS, Ctrl elsewhere (Windows/Linux).
  const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.platform || navigator.userAgent);

  function onkeydown(e: KeyboardEvent) {
    const cmd = isMac ? e.metaKey : e.ctrlKey;

    // Help overlay: ⌘⇧? (Mac) / Ctrl+Shift+? (Windows). HelpModal handles Escape-to-close.
    if (cmd && e.shiftKey && (e.key === "?" || e.code === "Slash")) {
      e.preventDefault();
      store.showHelp = !store.showHelp;
      return;
    }
    if (!store.tiddlReady) return;

    // Jump to Search + focus the box: ⌘F (Mac) / Ctrl+F (Windows).
    if (cmd && !e.altKey && !e.shiftKey && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      store.activeTab = "search";
      store.focusSearchNonce++;
      return;
    }
    // Previous / next tab — macOS ⌘⌥← / ⌘⌥→.
    if (e.metaKey && e.altKey && !e.ctrlKey) {
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        store.cycleTab(-1);
        return;
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        store.cycleTab(1);
        return;
      }
    }
    // Previous / next tab — Windows / cross-platform: Ctrl+PageUp / Ctrl+PageDown.
    if (e.ctrlKey && !e.metaKey && !e.altKey) {
      if (e.key === "PageUp") {
        e.preventDefault();
        store.cycleTab(-1);
        return;
      }
      if (e.key === "PageDown") {
        e.preventDefault();
        store.cycleTab(1);
        return;
      }
    }
    // Previous / next result category (in Search): Ctrl+Shift+Tab / Ctrl+Tab.
    if (e.ctrlKey && e.key === "Tab" && store.activeTab === "search") {
      e.preventDefault();
      store.cycleSearchCategory(e.shiftKey ? -1 : 1);
      return;
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="flex h-full flex-col" style="background: var(--bg)">
  <Header
    tiddl={store.tiddl}
    ffmpeg={store.ffmpeg}
    auth={store.auth}
    theme={store.theme}
    onToggleTheme={() => store.toggleTheme()}
    onSignIn={signIn}
    onSignOut={signOut}
    onRefreshAuth={refreshAuthToken}
    onHelp={() => (store.showHelp = true)}
  />

  {#if store.tiddlReady}
    <TabBar active={store.activeTab} onselect={(t) => (store.activeTab = t)} />

    {#if store.refreshingAuth}
      <div class="flex items-center gap-2 px-4 py-2 text-xs" style="background: var(--panel-soft); color: var(--muted)">
        <span>Refreshing your session…</span>
      </div>
    {:else if store.auth && !store.auth.authed}
      <div
        class="flex items-center gap-2 px-4 py-2 text-xs"
        style="background: color-mix(in srgb, var(--danger) 9%, transparent); color: var(--danger)"
      >
        <span>
          {store.auth.expired
            ? "Session expired — couldn't refresh automatically. Sign in again."
            : "Not signed in — Search, Favorites and downloads need sign-in."}
        </span>
        <button class="font-semibold underline" onclick={signIn}>Sign in</button>
      </div>
    {/if}

    <main class="min-h-0 flex-1 overflow-y-auto">
      {#if store.activeTab === "download"}
        <DownloadPanel />
      {:else if store.activeTab === "search"}
        <SearchPanel />
      {:else if store.activeTab === "library"}
        <LibraryPanel />
      {:else}
        <SettingsPanel onRecheck={refreshAll} />
      {/if}
    </main>

    <ActivityLog
      lines={store.log}
      collapsed={store.logCollapsed}
      ontoggle={() => (store.logCollapsed = !store.logCollapsed)}
      onclear={() => store.clearLog()}
    />
  {:else}
    <div class="min-h-0 flex-1"><FirstRunGate status={store.tiddl} onRecheck={refreshAll} /></div>
  {/if}
</div>

{#if store.showLogin}
  <LoginModal onclose={() => (store.showLogin = false)} />
{/if}

{#if confirm}
  <ConfirmDialog
    title={confirm.title}
    message={confirm.message}
    confirmLabel={confirm.confirmLabel}
    danger={confirm.danger}
    onconfirm={confirm.onconfirm}
    oncancel={() => (confirm = null)}
  />
{/if}

{#if store.showHelp}
  <HelpModal onclose={() => (store.showHelp = false)} />
{/if}
