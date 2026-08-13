import { createApp } from 'vue'
import { createPinia } from 'pinia'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import 'monaco-editor/min/vs/editor/editor.main.css'
import App from './App.vue'
import './style.css'
import { useThemeStore } from './stores/theme'

// Monaco 编辑器 Web Worker 环境（Vite 内联 worker）
;(globalThis as unknown as { MonacoEnvironment?: { getWorker: () => Worker } }).MonacoEnvironment = {
  getWorker: () => new editorWorker(),
}

// 挂载前应用主题（避免暗色默认值闪一下）
const pinia = createPinia()
useThemeStore(pinia).init()

createApp(App).use(pinia).mount('#app')
