<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as monaco from 'monaco-editor'
import { formatXml } from '../lib/xml'

const props = defineProps<{
  entryPath: string
  content: string
  language: string
  readOnly: boolean
  wordWrap: boolean
}>()

const emit = defineEmits<{
  (e: 'update:content', value: string): void
  (e: 'cursor', line: number, column: number): void
  (e: 'undo-state', canUndo: boolean, canRedo: boolean): void
  (e: 'word-wrap-change', enabled: boolean): void
}>()

const container = ref<HTMLDivElement | null>(null)
let editor: monaco.editor.IStandaloneCodeEditor | null = null
let model: monaco.editor.ITextModel | null = null
let suppressEmit = false
let disposables: monaco.IDisposable[] = []

function syncUndoState() {
  const m = editor?.getModel()
  emit('undo-state', m?.canUndo() ?? false, m?.canRedo() ?? false)
}

function attachModel() {
  model?.dispose()
  model = monaco.editor.createModel(props.content, props.language)
  editor?.setModel(model)
  suppressEmit = true
  editor?.updateOptions({ readOnly: props.readOnly })
  suppressEmit = false
  disposables.push(
    model.onDidChangeContent(() => {
      if (!suppressEmit) emit('update:content', model!.getValue())
      syncUndoState()
    }),
  )
  syncUndoState()
}

function formatDocument() {
  const ed = editor
  const m = ed?.getModel()
  if (!ed || !m) return
  const formatted = formatXml(ed.getValue())
  if (formatted === ed.getValue()) return
  const pos = ed.getPosition()
  ed.executeEdits('ooxml-format', [
    { range: m.getFullModelRange(), text: formatted, forceMoveMarkers: true },
  ])
  if (pos) ed.setPosition(pos)
  ed.focus()
}

onMounted(() => {
  if (!container.value) return
  editor = monaco.editor.create(container.value, {
    theme: 'vs-dark',
    fontSize: 13,
    fontFamily: "'SF Mono', 'JetBrains Mono', Menlo, Consolas, monospace",
    minimap: { enabled: true, scale: 1 },
    automaticLayout: true,
    scrollBeyondLastLine: false,
    wordWrap: props.wordWrap ? 'on' : 'off',
    tabSize: 2,
    renderWhitespace: 'none',
    fixedOverflowWidgets: true,
    padding: { top: 8 },
  })

  disposables.push(
    editor.onDidChangeCursorPosition((e) => {
      emit('cursor', e.position.lineNumber, e.position.column)
    }),
  )

  // 编辑器内 Alt+Z 切换换行时，同步外部按钮状态
  disposables.push(
    editor.onDidChangeConfiguration((e) => {
      if (e.hasChanged(monaco.editor.EditorOption.wordWrap)) {
        const enabled =
          editor!.getOption(monaco.editor.EditorOption.wordWrap) !== 'off'
        if (enabled !== props.wordWrap) emit('word-wrap-change', enabled)
      }
    }),
  )

  attachModel()
  editor.focus()

  if (props.language === 'xml') {
    disposables.push(
      editor.addAction({
        id: 'ooxml.format-xml',
        label: 'Format XML',
        keybindings: [monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF],
        run: formatDocument,
      }),
    )
  }
})

watch(
  () => props.entryPath,
  () => {
    if (!editor) return
    attachModel()
    editor.focus()
  },
)

watch(
  () => props.readOnly,
  (v) => editor?.updateOptions({ readOnly: v }),
)

watch(
  () => props.wordWrap,
  (v) => editor?.updateOptions({ wordWrap: v ? 'on' : 'off' }),
)

onBeforeUnmount(() => {
  disposables.forEach((d) => d.dispose())
  disposables = []
  model?.dispose()
  editor?.dispose()
})

defineExpose({
  undo: () => editor?.trigger('toolbar', 'undo', null),
  redo: () => editor?.trigger('toolbar', 'redo', null),
  format: formatDocument,
  focus: () => editor?.focus(),
})
</script>

<template>
  <div ref="container" class="monaco-host"></div>
</template>

<style scoped>
.monaco-host {
  width: 100%;
  height: 100%;
}
</style>
