<script lang="ts">
  import Pill from "./ui/Pill.svelte";
  import type { TiddlStatus, FfmpegStatus, AuthStatus } from "./types";
  let {
    tiddl,
    ffmpeg,
    auth,
  }: { tiddl: TiddlStatus | null; ffmpeg: FfmpegStatus | null; auth: AuthStatus | null } = $props();
</script>

<div class="flex items-center gap-2">
  {#if tiddl}
    {#if tiddl.found}
      <Pill variant="ok" title={tiddl.path}>tiddl ready</Pill>
    {:else}
      <Pill variant="danger" title={tiddl.error}>tiddl not found</Pill>
    {/if}
  {/if}
  {#if ffmpeg && !ffmpeg.found}
    <Pill variant="danger" title={ffmpeg.detail}>ffmpeg missing</Pill>
  {/if}
  {#if auth}
    {#if auth.authed}
      <Pill variant="ok" title={auth.detail}>signed in{auth.country_code ? ` (${auth.country_code})` : ""}</Pill>
    {:else if auth.expired}
      <Pill variant="danger" title={auth.detail}>token expired</Pill>
    {:else}
      <Pill variant="neutral" title={auth.detail}>not signed in</Pill>
    {/if}
  {/if}
</div>
