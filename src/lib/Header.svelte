<script lang="ts">
  import StatusPills from "./StatusPills.svelte";
  import type { TiddlStatus, FfmpegStatus, AuthStatus } from "./types";
  let {
    tiddl,
    ffmpeg,
    auth,
    theme,
    onToggleTheme,
    onSignIn,
    onSignOut,
    onRefreshAuth,
    onHelp,
  }: {
    tiddl: TiddlStatus | null;
    ffmpeg: FfmpegStatus | null;
    auth: AuthStatus | null;
    theme: "dark" | "light";
    onToggleTheme: () => void;
    onSignIn: () => void;
    onSignOut: () => void;
    onRefreshAuth: () => void;
    onHelp: () => void;
  } = $props();

  // Primary modifier glyph differs by platform (⌘ on macOS, Ctrl elsewhere).
  const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.platform || navigator.userAgent);
</script>

<header
  class="flex items-center justify-between border-b px-5 py-3"
  style="border-color: var(--border); background: var(--panel)"
>
  <div class="flex items-baseline gap-2">
    <h1 class="text-base font-semibold tracking-tight">tiddl-gui</h1>
    <span class="hidden text-xs sm:inline" style="color: var(--soft)">a front-end for the tiddl CLI</span>
  </div>
  <div class="flex items-center gap-2">
    <StatusPills {tiddl} {ffmpeg} {auth} />
    {#if auth?.authed}
      <button class="text-xs hover:underline" style="color: var(--soft)" onclick={onRefreshAuth}>refresh</button>
      <button class="text-xs hover:underline" style="color: var(--soft)" onclick={onSignOut}>sign out</button>
    {:else if tiddl?.found}
      <button
        class="rounded-lg px-2.5 py-1 text-xs font-semibold"
        style="background: var(--accent); color: #06231f"
        onclick={onSignIn}>Sign in</button
      >
    {/if}
    <button
      class="rounded-lg px-2 py-1 text-sm"
      style="background: var(--panel-soft)"
      title={`Keyboard shortcuts (${isMac ? "⌘⇧?" : "Ctrl+Shift+?"})`}
      aria-label="Keyboard shortcuts"
      onclick={onHelp}>?</button
    >
    <button
      class="rounded-lg px-2 py-1 text-sm"
      style="background: var(--panel-soft)"
      title="Toggle theme"
      aria-label="Toggle theme"
      onclick={onToggleTheme}>{theme === "dark" ? "☀" : "☾"}</button
    >
  </div>
</header>
