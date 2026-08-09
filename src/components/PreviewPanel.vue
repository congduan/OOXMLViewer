<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps<{
  /** docx | xlsx | pptx | ooxml */
  kind: string
  /** 整文件 base64 */
  base64: string
}>()

const container = ref<HTMLDivElement | null>(null)
const status = ref('')
let renderToken = 0
let pptxPreviewer: {
  preview: (file: ArrayBuffer) => Promise<unknown>
  destroy: () => void
} | null = null

function base64ToArrayBuffer(b64: string): ArrayBuffer {
  const bin = atob(b64)
  const buf = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i)
  return buf.buffer
}

async function render() {
  const el = container.value
  if (!el) return
  const token = ++renderToken
  pptxPreviewer?.destroy()
  pptxPreviewer = null
  el.innerHTML = ''
  status.value = 'Rendering…'
  try {
    const buffer = base64ToArrayBuffer(props.base64)
    if (props.kind === 'docx') {
      const { renderAsync } = await import('docx-preview')
      await renderAsync(buffer, el, el, {
        className: 'ooxml-docx',
        inWrapper: true,
        breakPages: true,
        ignoreWidth: false,
        ignoreHeight: false,
        ignoreFonts: false,
        renderHeaders: true,
        renderFooters: true,
      })
    } else if (props.kind === 'xlsx') {
      const XLSX = await import('xlsx')
      const wb = XLSX.read(buffer, { type: 'array' })
      el.innerHTML = wb.SheetNames.map((name, i) => {
        const ws = wb.Sheets[name]
        const html = XLSX.utils.sheet_to_html(ws, { id: `sheet-${i}` })
        return `<div class="sheet-block"><div class="sheet-name">${name}</div>${html}</div>`
      }).join('')
    } else if (props.kind === 'pptx') {
      const { init } = await import('pptx-preview')
      pptxPreviewer = init(el, { mode: 'list' })
      await pptxPreviewer.preview(buffer)
    } else {
      el.innerHTML =
        '<div class="preview-note">Live preview is not available for this file type.</div>'
      return
    }
    if (token === renderToken) status.value = ''
  } catch (e) {
    if (token === renderToken) {
      el.innerHTML = ''
      status.value = `Preview failed: ${e}`
    }
  }
}

watch(
  () => [props.kind, props.base64] as const,
  () => {
    if (props.base64) void render()
  },
)

onMounted(() => {
  if (props.base64) void render()
})

onBeforeUnmount(() => {
  pptxPreviewer?.destroy()
  pptxPreviewer = null
})
</script>

<template>
  <div class="preview-panel">
    <div ref="container" class="preview-body"></div>
    <div v-if="status" class="preview-status">{{ status }}</div>
  </div>
</template>

<style scoped>
.preview-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}

.preview-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 14px;
  background: #1a1a1c;
}

.preview-status {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 5;
  padding: 6px 12px;
  font-size: 11.5px;
  color: #f0b6b6;
  background: color-mix(in srgb, #3a1e1e 92%, transparent);
  border-top: 1px solid #6b3a3a;
}

.preview-note {
  color: var(--fg-dim);
  font-size: 12.5px;
  padding: 20px;
  text-align: center;
}

/* xlsx 工作表 */
.sheet-block {
  margin-bottom: 16px;
}

.sheet-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--fg-dim);
  margin-bottom: 6px;
}

.sheet-block table {
  border-collapse: collapse;
  background: #fff;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
}

/* docx 页面容器：外层 wrapper 透明（其默认是灰色底 + 30px 内边距），
   仅保留单个白色页面卡片（docx-preview 的类名为 {className}-wrapper） */
.preview-body :deep(.docx-wrapper),
.preview-body :deep(.ooxml-docx-wrapper) {
  background: transparent !important;
  padding: 0 !important;
  align-items: center;
}

.preview-body :deep(.docx-wrapper section),
.preview-body :deep(.ooxml-docx-wrapper section) {
  margin-bottom: 14px;
}
</style>
