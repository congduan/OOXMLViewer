<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import { isTextualPath } from '../lib/backend'

const props = defineProps<{ parentDir: string }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const store = useWorkspaceStore()
const entryPath = ref('')
const content = ref('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<root/>\n')
const error = ref('')

onMounted(() => {
  entryPath.value = props.parentDir
    ? `${props.parentDir}/new-file.xml`
    : 'new-file.xml'
})

const validPath = computed(() => {
  const p = entryPath.value.trim()
  if (!p) return 'Enter an entry path'
  if (p.endsWith('/')) return 'Path cannot end with "/"'
  if (!isTextualPath(p)) return 'Text files only (.xml / .rels / .json, etc.)'
  return ''
})

async function create() {
  const msg = validPath.value
  if (msg) {
    error.value = msg
    return
  }
  const p = entryPath.value.trim()
  if (store.loading) return
  await store.addEntry(p, content.value)
  emit('close')
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <header class="dialog-title">Add File</header>

      <label class="field">
        <span class="field-label">Entry path</span>
        <input
          v-model="entryPath"
          class="input"
          type="text"
          placeholder="e.g. word/custom.xml"
          spellcheck="false"
          @keydown.enter="create"
        />
        <span v-if="validPath" class="field-hint warn">{{ validPath }}</span>
      </label>

      <label class="field">
        <span class="field-label">Content</span>
        <textarea
          v-model="content"
          class="input textarea"
          rows="10"
          spellcheck="false"
        ></textarea>
      </label>

      <div v-if="error" class="dialog-error">{{ error }}</div>

      <footer class="dialog-actions">
        <button class="btn" @click="emit('close')">Cancel</button>
        <button class="btn primary" :disabled="!!validPath || store.loading" @click="create">
          {{ store.loading ? 'Saving…' : 'Create' }}
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 300;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
}

.dialog {
  width: 520px;
  max-width: 90vw;
  background: var(--popup-bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 16px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.field-label {
  font-size: 11.5px;
  color: var(--fg-dim);
}

.input {
  width: 100%;
  padding: 7px 10px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--fg);
  font-size: 12.5px;
  font-family: 'SF Mono', Menlo, Consolas, monospace;
  outline: none;
}

.input:focus {
  border-color: var(--accent);
}

.textarea {
  resize: vertical;
  min-height: 160px;
  line-height: 1.5;
}

.field-hint {
  font-size: 11px;
  color: var(--fg-dim);
}

.field-hint.warn {
  color: var(--warn);
}

.dialog-error {
  font-size: 12px;
  color: var(--danger);
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
