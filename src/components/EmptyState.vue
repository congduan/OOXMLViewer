<script setup lang="ts">
defineProps<{ dragging: boolean }>()
defineEmits<{ (e: 'open'): void }>()
</script>

<template>
  <div class="empty-state" :class="{ dragging }">
    <div class="drop-icon">
      <svg viewBox="0 0 48 48" width="64" height="64">
        <path
          d="M14 10a4 4 0 0 0-4 4v20a4 4 0 0 0 4 4h20a4 4 0 0 0 4-4V20l-8-8H14zm2 6h11l6 6v12a2 2 0 0 1-2 2H16a2 2 0 0 1-2-2V18a2 2 0 0 1 2-2z"
          fill="currentColor"
        />
        <path
          d="M24 21v9m0 0l-3.5-3.5M24 30l3.5-3.5"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </div>
    <h2 class="title">Drop an OOXML file</h2>
    <p class="hint">
      Supports .docx / .xlsx / .pptx — browse the internal structure, edit XML inline, and write
      changes back to the file
    </p>
    <button class="btn primary" @click="$emit('open')">Choose File…</button>
  </div>
</template>

<style scoped>
.empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--fg-dim);
  border-radius: 10px;
}

.empty-state.dragging {
  outline: 2px dashed var(--accent);
  outline-offset: -10px;
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  color: var(--accent);
}

.drop-icon {
  color: var(--fg-dim);
  transition: color 0.15s;
}

.empty-state.dragging .drop-icon {
  color: var(--accent);
}

.title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: var(--fg);
}

.hint {
  margin: 0 0 6px;
  font-size: 13px;
}
</style>
