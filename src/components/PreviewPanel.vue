<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { DocxEditorHandle } from '@eigenpal/docx-editor-vue'
import { useThemeStore } from '../stores/theme'

const props = defineProps<{
  /** docx | xlsx | pptx | ooxml */
  kind: string
  /** 整文件 base64 */
  base64: string
}>()

const themeStore = useThemeStore()

const container = ref<HTMLDivElement | null>(null)
const status = ref('')
const sheetNames = ref<string[]>([])
const sheetHtmls = ref<string[]>([])
const activeSheet = ref(0)
let renderToken = 0
let docxHandle: DocxEditorHandle | null = null
let docxResizeObserver: ResizeObserver | null = null
let pptxPreviewer: {
  preview: (file: ArrayBuffer) => Promise<unknown>
  destroy: () => void
} | null = null

/** 让 docx 预览按面板宽度自适应缩放（zoom = 视口宽 / 页面自然宽） */
function fitDocxToWidth(handle: DocxEditorHandle, root: HTMLElement) {
  const viewport = root.querySelector('.docx-editor-vue__pages-viewport')
  const page = root.querySelector('.docx-editor-vue__pages')?.firstElementChild
  const vw = viewport instanceof HTMLElement ? viewport.clientWidth : 0
  const pageW = page instanceof HTMLElement ? page.offsetWidth : 0
  if (vw > 0 && pageW > 0) {
    handle.setZoom(Math.min(1.5, Math.max(0.4, (vw - 24) / pageW)))
  }
}

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
  docxHandle?.destroy()
  docxHandle = null
  docxResizeObserver?.disconnect()
  docxResizeObserver = null
  pptxPreviewer?.destroy()
  pptxPreviewer = null
  // xlsx 由模板渲染（标签页 + 表格），其余类型注入 container
  if (props.kind !== 'xlsx') el.innerHTML = ''
  status.value = 'Rendering…'
  try {
    const buffer = base64ToArrayBuffer(props.base64)
    if (props.kind === 'docx') {
      const { renderAsync } = await import('@eigenpal/docx-editor-vue')
      await import('@eigenpal/docx-editor-vue/styles.css')
      docxHandle = await renderAsync(buffer, el, {
        mode: 'viewing',
        readOnly: true,
        showToolbar: false,
        showMenuBar: false,
        showRuler: false,
        showOutline: false,
        showOutlineButton: false,
        showZoomControl: false,
        colorMode: themeStore.theme,
        className: 'ooxml-docx',
      })
      // 自适应宽度：初始缩放 + 面板尺寸变化时重新适配
      fitDocxToWidth(docxHandle, el)
      docxResizeObserver = new ResizeObserver(() => {
        if (docxHandle) fitDocxToWidth(docxHandle, el)
      })
      docxResizeObserver.observe(el)
      // 分页/字体加载完成后微调一次
      window.setTimeout(() => {
        if (token === renderToken && docxHandle) fitDocxToWidth(docxHandle, el)
      }, 200)
    } else if (props.kind === 'xlsx') {
      const XLSX = await import('xlsx')
      const wb = XLSX.read(buffer, { type: 'array' })
      sheetNames.value = wb.SheetNames
      sheetHtmls.value = wb.SheetNames.map((name, i) =>
        XLSX.utils.sheet_to_html(wb.Sheets[name], { id: `sheet-${i}` }),
      )
      activeSheet.value = 0
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

// docx 编辑器的 colorMode 在挂载时固定，切换主题时同步其 .dark 类
watch(
  () => themeStore.theme,
  () => {
    container.value
      ?.querySelector('.ooxml-docx')
      ?.classList.toggle('dark', themeStore.theme === 'dark')
  },
)

onMounted(() => {
  if (props.base64) void render()
})

onBeforeUnmount(() => {
  docxHandle?.destroy()
  docxHandle = null
  docxResizeObserver?.disconnect()
  docxResizeObserver = null
  pptxPreviewer?.destroy()
  pptxPreviewer = null
})
</script>

<template>
  <div class="preview-panel">
    <div ref="container" class="preview-body" :class="{ flush: props.kind === 'xlsx' }">
      <template v-if="props.kind === 'xlsx' && sheetNames.length">
        <div class="sheet-tabs">
          <button
            v-for="(name, i) in sheetNames"
            :key="name"
            class="sheet-tab"
            :class="{ active: i === activeSheet }"
            @click="activeSheet = i"
          >
            {{ name }}
          </button>
        </div>
        <div class="sheet-viewport">
          <div class="sheet-table" v-html="sheetHtmls[activeSheet]"></div>
        </div>
      </template>
    </div>
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
  background: var(--preview-bg);
  transition: background 0.15s;
}

/* xlsx 预览时去掉内边距，让标签栏贴到顶部 */
.preview-body.flush {
  padding: 0;
}

.preview-status {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 5;
  padding: 6px 12px;
  font-size: 11.5px;
  color: var(--status-err-fg);
  background: var(--status-err-bg);
  border-top: 1px solid var(--status-err-border);
}

.preview-note {
  color: var(--fg-dim);
  font-size: 12.5px;
  padding: 20px;
  text-align: center;
}

/* xlsx 预览：工作表标签页 + 表格 */
.sheet-tabs {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  gap: 2px;
  overflow-x: auto;
  padding: 0 6px;
  background: var(--preview-bg);
}

.sheet-tab {
  border: 1px solid transparent;
  border-bottom: none;
  background: var(--preview-tab-bg);
  color: var(--preview-tab-fg);
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 6px 6px 0 0;
  white-space: nowrap;
}

.sheet-tab:hover {
  color: var(--preview-tab-fg-hover);
}

.sheet-tab.active {
  background: #fff;
  color: #1a1a1c;
  font-weight: 600;
}

.sheet-viewport {
  padding: 10px;
}

.sheet-table {
  background: #fff;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
  display: inline-block;
  min-width: 100%;
}

.sheet-table :deep(table) {
  border-collapse: collapse;
  width: 100%;
}

.sheet-table :deep(th),
.sheet-table :deep(td) {
  border: 1px solid #d3d9e0;
  padding: 4px 8px;
  font-size: 12.5px;
  color: #1a1a1c;
  white-space: nowrap;
}

.sheet-table :deep(th) {
  background: #f3f5f7;
  color: #5a6472;
  font-weight: 600;
  text-align: center;
}

.sheet-table :deep(tbody th) {
  background: #eef1f4;
}

/* docx 预览（docx-editor 只读模式，内部自带分页滚动） */
.preview-body :deep(.ooxml-docx) {
  height: 100%;
}

/* 页面水平居中（编辑器默认左对齐） */
.preview-body :deep(.docx-editor-vue__pages) {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.preview-body :deep(.layout-page) {
  margin: 0 auto;
}
</style>
