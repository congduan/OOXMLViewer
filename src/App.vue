<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useWorkspaceStore } from './stores/workspace'
import { formatSize, languageForPath, onFileDrop, pickFile, revealInFolder, extractEntry, pickDirectory } from './lib/backend'
import EntryTree from './components/EntryTree.vue'
import MonacoEditor from './components/MonacoEditor.vue'
import EmptyState from './components/EmptyState.vue'
import AddFileDialog from './components/AddFileDialog.vue'
import PreviewPanel from './components/PreviewPanel.vue'

const store = useWorkspaceStore()

const dragging = ref(false)
const cursor = ref({ line: 0, column: 0 })
const addDialog = ref<{ parentDir: string } | null>(null)
const canUndo = ref(false)
const canRedo = ref(false)
const wordWrap = ref(true)
const editorRef = ref<InstanceType<typeof MonacoEditor> | null>(null)
let unlisten: (() => void) | null = null

const kindLabel = computed(() => {
  const map: Record<string, string> = { docx: 'DOCX', xlsx: 'XLSX', pptx: 'PPTX' }
  return map[store.file?.kind ?? ''] ?? 'OOXML'
})

const language = computed(() =>
  store.current ? languageForPath(store.current.entry.path) : 'plaintext',
)

/** 是否可编辑文本且编辑器已挂载 */
const editorActive = computed(
  () => !!store.current && store.currentEditable && !!editorRef.value,
)
const canFormat = computed(() => editorActive.value && language.value === 'xml')

const toolbarHint = computed(() => {
  if (store.loading) return 'Processing…'
  if (!store.file) return 'Drop an OOXML file or click "Open"'
  if (!store.current) return 'Select a file from the left panel'
  if (store.current.kind === 'image') return 'Image preview · Read-only'
  if (store.current.kind === 'binary') return 'Binary file · Read-only preview'
  if (store.dirty) return 'Unsaved changes'
  return 'Ready'
})

async function onPick() {
  const path = await pickFile()
  if (path) await store.openFile(path)
}

function onCursor(line: number, column: number) {
  cursor.value = { line, column }
}

function onUndoState(u: boolean, r: boolean) {
  canUndo.value = u
  canRedo.value = r
}

function onAddFile(parentDir: string) {
  addDialog.value = { parentDir }
}

function onDeleteFile(path: string) {
  const ok = window.confirm(
    `Delete entry "${path}" from the file?\nThe deletion takes effect after you click "Save". You can restore it before saving, or use "Restore" after saving.`,
  )
  if (ok) store.removeEntry(path)
}

async function onExtractFile(path: string) {
  if (!store.file) return
  const dest = await pickDirectory('Choose a destination folder')
  if (!dest) return
  try {
    const size = await extractEntry(store.file.file_path, path, dest)
    store.showToast(`Exported ${path} → ${dest} (${formatSize(size)})`)
  } catch (e) {
    store.setError(`Export failed: ${e}`)
  }
}

async function onReveal() {
  if (!store.file) return
  try {
    await revealInFolder(store.file.file_path)
  } catch (e) {
    store.setError(`Failed to reveal file in folder: ${e}`)
  }
}

function onCloseFile() {
  if (store.dirty) {
    const ok = window.confirm('This entry has unsaved changes. Closing the file will discard them. Continue?')
    if (!ok) return
  }
  store.closeFile()
}

/** 事件目标是否位于 Monaco 编辑器内部（编辑器聚焦时由其自行处理快捷键） */
function insideMonaco(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest('.monaco-editor')
}

function onKeydown(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey
  const key = e.key.toLowerCase()
  if (mod && key === 's') {
    e.preventDefault()
    void store.save()
  } else if (mod && key === 'o') {
    e.preventDefault()
    void onPick()
  } else if (mod && key === 'w') {
    e.preventDefault()
    onCloseFile()
  } else if (mod && e.shiftKey && key === 'z') {
    if (!insideMonaco(e.target)) {
      e.preventDefault()
      editorRef.value?.redo()
    }
  } else if (mod && key === 'z') {
    if (!insideMonaco(e.target)) {
      e.preventDefault()
      editorRef.value?.undo()
    }
  }
}

onMounted(async () => {
  unlisten = await onFileDrop({
    over: () => (dragging.value = true),
    leave: () => (dragging.value = false),
    drop: (paths) => {
      dragging.value = false
      if (paths.length) void store.openFile(paths[0])
    },
  })
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  unlisten?.()
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="app">
    <header class="titlebar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <svg viewBox="0 0 24 24" width="18" height="18" class="brand-icon">
          <path
            d="M6 2a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6H6zm9 1.5L19.5 8H15V3.5zM8 12h8v1.5H8V12zm0 3.5h8V17H8v-1.5z"
            fill="currentColor"
          />
        </svg>
        <span>OOXML Viewer</span>
      </div>
      <div v-if="store.file" class="file-meta" data-tauri-drag-region>
        <span class="file-name" :title="store.file.file_path">
          {{ store.file.file_name }}
        </span>
        <span class="kind-badge">{{ kindLabel }}</span>
        <span class="file-size">{{ formatSize(store.file.file_size) }}</span>
        <span v-if="store.dirty" class="dirty-text">● Unsaved</span>
      </div>
    </header>

    <nav class="toolbar">
      <div class="toolbar-group">
        <button class="btn" title="Open file (⌘O)" @click="onPick">
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M9 2h4v4h-1.5V4.2L7.2 8.5 6.1 7.4l4.3-4.3H9V2zm-5 2h3v1.5H4.5v8h7V9H13v4a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"
              fill="currentColor"
            />
          </svg>
          Open
        </button>
        <button
          class="btn"
          :class="{ primary: store.dirty }"
          title="Save (⌘S)"
          :disabled="!store.file || !store.dirty || store.loading"
          @click="store.save()"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M4 1.5h7l2.5 2.5v10a.5.5 0 0 1-.5.5H4a.5.5 0 0 1-.5-.5v-12A.5.5 0 0 1 4 1.5zM6 3v3h4V3H6zm1 9h2V8H7v4z"
              fill="currentColor"
            />
          </svg>
          Save
        </button>
        <button
          class="btn"
          title="Close file (⌘W)"
          :disabled="!store.file"
          @click="onCloseFile"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M4 4l8 8M12 4l-8 8"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
            />
          </svg>
          Close
        </button>
        <button
          class="btn"
          title="Reveal the file in its folder"
          :disabled="!store.file"
          @click="onReveal"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M1.5 4.5h4.2l1.4 1.5h7.4a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-12a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.3"
              stroke-linejoin="round"
            />
            <circle cx="10.5" cy="9.5" r="1.8" fill="none" stroke="currentColor" stroke-width="1.3" />
            <path
              d="M11.9 10.9l1.3 1.3"
              stroke="currentColor"
              stroke-width="1.3"
              stroke-linecap="round"
            />
          </svg>
          Reveal in Folder
        </button>
        <button
          class="btn"
          title="Restore the last saved version (revert deletes/edits)"
          :disabled="!store.file || !store.hasBackup || store.loading"
          @click="store.restoreLastSave()"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M3 4.5V8h3.5M3 8a5 5 0 1 0 2.1-3.8"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <path
              d="M8 5.5V8l2 1.2"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
          Restore
        </button>
        <button
          class="btn"
          :class="{ active: store.previewOpen }"
          title="Toggle the live preview panel"
          :disabled="!store.previewReady"
          @click="store.togglePreview()"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M2 8s2.5-4.5 6-4.5S14 8 14 8s-2.5 4.5-6 4.5S2 8 2 8z"
              fill="none"
              stroke="currentColor"
              stroke-width="1.3"
            />
            <circle cx="8" cy="8" r="2" fill="none" stroke="currentColor" stroke-width="1.3" />
          </svg>
          Preview
        </button>
      </div>

      <span class="toolbar-sep"></span>

      <div class="toolbar-group">
        <button
          class="btn"
          title="Undo (⌘Z)"
          :disabled="!editorActive || !canUndo"
          @click="editorRef?.undo()"
        >
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
          Undo
        </button>
        <button
          class="btn"
          title="Redo (⇧⌘Z)"
          :disabled="!editorActive || !canRedo"
          @click="editorRef?.redo()"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M10 4l3.5 3.5L10 11m3.5-3.5h-6a4.5 4.5 0 0 0 0 9H9"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          Redo
        </button>
        <button
          class="btn"
          title="Format XML (⇧⌥F)"
          :disabled="!canFormat"
          @click="editorRef?.format()"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M5 5.5L2.5 8 5 10.5M11 5.5L13.5 8 11 10.5M9 3L7 13"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          Format
        </button>
        <span class="toolbar-sep"></span>
        <button
          class="btn"
          :class="{ active: wordWrap }"
          title="Word Wrap"
          @click="wordWrap = !wordWrap"
        >
          <svg viewBox="0 0 16 16" width="13" height="13">
            <path
              d="M2 4h8a3 3 0 0 1 0 6H6m0 0l2-2m-2 2l2 2"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          Word Wrap
        </button>
      </div>

      <span class="toolbar-spacer"></span>
      <span class="toolbar-hint">{{ toolbarHint }}</span>
    </nav>

    <main class="body">
      <aside v-if="store.file" class="sidebar">
        <EntryTree
          @add-file="onAddFile"
          @delete-file="onDeleteFile"
          @restore-file="store.restoreEntry"
          @extract-file="onExtractFile"
        />
      </aside>

      <section class="content">
        <EmptyState v-if="!store.file" :dragging="dragging" @open="onPick" />

        <template v-else-if="store.file">
          <div class="content-split">
            <template v-if="store.current">
              <div class="editor-wrap">
                <MonacoEditor
                  v-if="store.current.kind !== 'image'"
                  ref="editorRef"
                  :entry-path="store.current.entry.path"
                  :content="store.current.content"
                  :language="language"
                  :read-only="!store.currentEditable"
                  :word-wrap="wordWrap"
                  @update:content="store.updateContent"
                  @cursor="onCursor"
                  @undo-state="onUndoState"
                  @word-wrap-change="(v) => (wordWrap = v)"
                />
                <div v-else class="image-preview">
                  <img :src="store.current.content" alt="Preview" draggable="false" />
                </div>
                <div v-if="store.current.kind === 'binary'" class="entry-banner">
                  Binary file · only the first 2 KB shown as hex, read-only
                </div>
              </div>
            </template>
            <div v-else class="content-placeholder">
              {{ store.loading ? 'Loading…' : 'Select a file from the left panel' }}
            </div>

            <div
              v-if="store.previewOpen && store.previewReady"
              class="preview-column"
            >
              <div class="preview-header">
                <span class="preview-title">Preview</span>
                <button
                  class="preview-collapse"
                  title="Collapse preview"
                  @click="store.togglePreview()"
                >
                  <svg viewBox="0 0 16 16" width="12" height="12">
                    <path
                      d="M10 3l-4 5 4 5"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="1.6"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                </button>
              </div>
              <PreviewPanel :kind="store.file.kind" :base64="store.previewB64" />
            </div>
          </div>
          <footer v-if="store.current" class="statusbar">
            <span class="sb-path" :title="store.current.entry.path">
              {{ store.current.entry.path }}
            </span>
            <span class="sb-spacer"></span>
            <span v-if="store.current.kind === 'text'" class="sb-item">
              Ln {{ cursor.line }}, Col {{ cursor.column }}
            </span>
            <span class="sb-item">
              {{
                store.current.kind === 'text'
                  ? 'UTF-8'
                  : store.current.kind === 'image'
                    ? 'Image'
                    : 'Binary'
              }}
            </span>
            <span class="sb-item">{{ formatSize(store.current.entry.size) }}</span>
            <span v-if="store.dirty" class="sb-item dirty-text">● Unsaved</span>
          </footer>
        </template>
      </section>
    </main>

    <div v-if="store.toast" class="toast">{{ store.toast }}</div>
    <div v-if="store.error" class="error-bar">
      <span>{{ store.error }}</span>
      <button class="error-close" @click="store.setError('')">✕</button>
    </div>

    <AddFileDialog
      v-if="addDialog"
      :parent-dir="addDialog.parentDir"
      @close="addDialog = null"
    />
  </div>
</template>

<style scoped>
.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
  color: var(--fg);
}

/* 顶栏 */
.titlebar {
  flex: none;
  height: 44px;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 12px;
  background: var(--titlebar-bg);
  border-bottom: 1px solid var(--border);
  -webkit-user-select: none;
  user-select: none;
}

.brand {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 13px;
  font-weight: 600;
  color: var(--fg);
}

.brand-icon {
  color: var(--accent);
}

.file-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.file-name {
  font-size: 12.5px;
  color: var(--fg-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 340px;
}

.kind-badge {
  flex: none;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.5px;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--accent-dim);
  color: var(--accent);
}

.file-size {
  flex: none;
  font-size: 11px;
  color: var(--fg-dim);
}

/* 工具栏 */
.toolbar {
  flex: none;
  height: 40px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  background: var(--panel-bg);
  border-bottom: 1px solid var(--border);
  -webkit-user-select: none;
  user-select: none;
}

.toolbar-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.toolbar-sep {
  width: 1px;
  height: 20px;
  background: var(--border);
  flex: none;
}

.toolbar-spacer {
  flex: 1;
}

.toolbar-hint {
  font-size: 11.5px;
  color: var(--fg-dim);
}

/* 主体 */
.body {
  flex: 1;
  display: flex;
  min-height: 0;
}

.sidebar {
  flex: none;
  width: 290px;
  border-right: 1px solid var(--border);
  background: var(--panel-bg);
  display: flex;
  min-height: 0;
}

.content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--editor-bg);
  min-height: 0;
}

/* 编辑器 + 右侧预览 水平分栏 */
.content-split {
  flex: 1;
  min-height: 0;
  display: flex;
}

.preview-column {
  flex: none;
  width: 46%;
  min-width: 280px;
  max-width: 60%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-left: 1px solid var(--border);
  background: var(--panel-bg);
}

.preview-header {
  flex: none;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 6px 0 12px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;
  color: var(--fg-dim);
  -webkit-user-select: none;
  user-select: none;
}

.preview-title {
  font-weight: 600;
  letter-spacing: 0.3px;
}

.preview-collapse {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--fg-dim);
  cursor: pointer;
}

.preview-collapse:hover {
  background: var(--hover);
  color: var(--accent);
}

.editor-wrap {
  flex: 1;
  min-height: 0;
  position: relative;
}

.entry-banner {
  position: absolute;
  top: 10px;
  right: 12px;
  z-index: 5;
  font-size: 11px;
  color: var(--fg-dim);
  background: color-mix(in srgb, var(--panel-bg) 92%, transparent);
  border: 1px solid var(--border);
  border-radius: 5px;
  padding: 4px 10px;
}

/* 图片预览 */
.image-preview {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background-color: #1a1a1c;
  background-image:
    linear-gradient(45deg, #202023 25%, transparent 25%, transparent 75%, #202023 75%),
    linear-gradient(45deg, #202023 25%, transparent 25%, transparent 75%, #202023 75%);
  background-size: 22px 22px;
  background-position: 0 0, 11px 11px;
}

.image-preview img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  border-radius: 4px;
  user-select: none;
}

.content-placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--fg-dim);
  font-size: 13px;
}

/* 状态栏 */
.statusbar {
  flex: none;
  height: 24px;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 10px;
  font-size: 11px;
  color: var(--fg-dim);
  background: var(--titlebar-bg);
  border-top: 1px solid var(--border);
}

.sb-path {
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sb-spacer {
  flex: 1;
}

.dirty-text {
  color: #e5c07b;
}

/* 提示与错误 */
.toast {
  position: fixed;
  top: 52px;
  right: 16px;
  z-index: 100;
  font-size: 12.5px;
  padding: 8px 14px;
  border-radius: 6px;
  background: #1e2a1e;
  color: #8fd18f;
  border: 1px solid #3a6b3a;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
}

.error-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 16px;
  font-size: 12.5px;
  color: #f0b6b6;
  background: #3a1e1e;
  border-top: 1px solid #6b3a3a;
}

.error-close {
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  font-size: 12px;
  padding: 2px 6px;
}
</style>
