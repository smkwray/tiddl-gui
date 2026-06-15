<script lang="ts">
  import Modal from "./ui/Modal.svelte";
  let {
    title,
    message,
    confirmLabel = "Confirm",
    danger = false,
    onconfirm,
    oncancel,
  }: {
    title: string;
    message: string;
    confirmLabel?: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  } = $props();
</script>

<Modal onclose={oncancel}>
  <div class="p-5">
    <h2 class="mb-2 text-base font-semibold">{title}</h2>
    <p class="mb-4 text-sm leading-relaxed" style="color: var(--muted)">{message}</p>
    <div class="flex justify-end gap-2">
      <!-- svelte-ignore a11y_autofocus -->
      <button
        autofocus
        class="rounded-lg px-3 py-1.5 text-sm"
        style="background: var(--panel-soft); color: var(--text)"
        onclick={oncancel}>Cancel</button
      >
      <button
        class="rounded-lg px-3 py-1.5 text-sm font-semibold"
        style={danger ? "background: var(--danger); color: #fff" : "background: var(--accent); color: #06231f"}
        onclick={onconfirm}>{confirmLabel}</button
      >
    </div>
  </div>
</Modal>
