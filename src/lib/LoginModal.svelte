<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import Modal from "./ui/Modal.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let starting = $state(true);
  let err = $state("");
  let copied = $state(false);
  let link = $state("");

  const flow = $derived(store.authFlow);
  // Keep the verification link visible through polling — later events overwrite
  // authFlow.uri with undefined, so latch the last non-empty value.
  $effect(() => {
    if (flow?.uri) link = flow.uri;
  });

  onMount(async () => {
    try {
      await invoke("auth_login");
    } catch (e) {
      if (String(e) !== "cancelled") err = String(e);
    } finally {
      starting = false;
    }
  });

  function openLink() {
    if (link) invoke("open_url", { url: link }).catch((e) => (err = String(e)));
  }
  async function copy() {
    if (!link) return;
    try {
      await navigator.clipboard.writeText(link);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* clipboard blocked — user can select the text */
    }
  }
  function cancel() {
    invoke("cancel_download", { id: 0 }).catch(() => {});
    onclose();
  }
</script>

<Modal onclose={cancel}>
  <div class="p-5">
    <div class="mb-3 flex items-center justify-between">
      <h2 class="text-base font-semibold">Sign in</h2>
      <button onclick={cancel} aria-label="Close" style="color: var(--soft)">✕</button>
    </div>

    {#if err}
      <p class="text-sm" style="color: var(--danger)">{err}</p>
    {:else if flow?.phase === "success" || flow?.phase === "already"}
      <p class="text-sm" style="color: var(--ok)">✓ Signed in!</p>
    {:else if flow?.phase === "expired"}
      <p class="text-sm" style="color: var(--danger)">Authentication time expired. Close and try again.</p>
    {:else if link}
      <p class="mb-2 text-sm" style="color: var(--muted)">Open this link, authorize, then return here:</p>
      <div class="mb-3 flex items-center gap-2">
        <code class="flex-1 truncate rounded border px-2 py-1.5 text-xs" style="border-color: var(--border); background: var(--bg)">{link}</code>
        <button class="rounded px-2 py-1.5 text-xs" style="background: var(--panel-soft)" onclick={copy}>{copied ? "✓" : "Copy"}</button>
        <button class="rounded px-2 py-1.5 text-xs font-semibold" style="background: var(--accent); color: #06231f" onclick={openLink}>Open</button>
      </div>
      <p class="text-sm" style="color: var(--muted)">{flow?.phase === "polling" ? flow.text : "Waiting for authorization…"}</p>
    {:else}
      <p class="text-sm" style="color: var(--muted)">{starting ? "Starting sign-in…" : flow?.text ?? "Working…"}</p>
    {/if}

    <div class="mt-4 flex justify-end">
      <button class="rounded-lg px-3 py-1.5 text-sm" style="background: var(--panel-soft); color: var(--text)" onclick={cancel}>
        {flow?.phase === "success" || flow?.phase === "already" ? "Close" : "Cancel"}
      </button>
    </div>
  </div>
</Modal>
