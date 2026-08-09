<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'

defineProps<{
  x: number
  y: number
  isDir: boolean
  pendingDelete: boolean
  pendingAdd: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'add'): void
  (e: 'delete'): void
  (e: 'restore'): void
  (e: 'copy'): void
  (e: 'extract'): void
}>()

function close() {
  emit('close')
}

function onDocClick() {
  close()
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') close()
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
  document.addEventListener('keydown', onKey)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick)
  document.removeEventListener('keydown', onKey)
})
</script>

<template>
  <div class="ctx-menu" :style="{ left: `${x}px`, top: `${y}px` }" @click.stop>
    <button class="item" @click="emit('add')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
      {{ isDir ? 'Add file in this folder…' : 'Add file in same folder…' }}
    </button>
    <button v-if="pendingDelete" class="item" @click="emit('restore')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path
          d="M6 4L2.5 7.5 6 11M3 7.5h6a4.5 4.5 0 0 1 0 9H7"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      Undo delete
    </button>
    <button v-else-if="pendingAdd" class="item" @click="emit('restore')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path
          d="M3 4.5v3.5h3.5M3 8a5 5 0 1 0 2.1-3.8M8 5.5v3l1.8 1"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      Undo add
    </button>
    <button v-else-if="!isDir" class="item danger" @click="emit('delete')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path
          d="M4.5 5h7l-.4 7.5a1 1 0 0 1-1 .9H5.9a1 1 0 0 1-1-.9L4.5 5zm2-2.5h3l.5 1H4.5l.5-1zM6.5 7v4M9.5 7v4"
          fill="none"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
      </svg>
      Delete file
    </button>
    <button class="item" @click="emit('copy')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path
          d="M5.5 4.5V3a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-1.5M4.5 5h5a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z"
          fill="none"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
      </svg>
      Copy path
    </button>
    <button v-if="!isDir" class="item" @click="emit('extract')">
      <svg viewBox="0 0 16 16" width="13" height="13">
        <path
          d="M8 2v8m0 0L5.5 7.5M8 10l2.5-2.5M3.5 11v1.5a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1V11"
          fill="none"
          stroke="currentColor"
          stroke-width="1.4"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      Export to…
    </button>
  </div>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  z-index: 200;
  min-width: 170px;
  padding: 4px;
  background: #252526;
  border: 1px solid var(--border);
  border-radius: 6px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
  display: flex;
  flex-direction: column;
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 10px;
  border: none;
  background: transparent;
  color: var(--fg);
  font-size: 12.5px;
  border-radius: 4px;
  cursor: pointer;
  text-align: left;
  white-space: nowrap;
}

.item:hover {
  background: var(--hover);
}

.item.danger {
  color: var(--danger);
}

.item.danger:hover {
  background: rgba(224, 108, 117, 0.15);
}
</style>
