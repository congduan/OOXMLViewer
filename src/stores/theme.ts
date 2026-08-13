import { defineStore } from 'pinia'
import { ref } from 'vue'

export type ThemeMode = 'dark' | 'light'

const STORAGE_KEY = 'ooxml-viewer-theme'

function systemTheme(): ThemeMode {
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
}

/** 全局日夜间模式：localStorage 持久化，未设置时跟随系统 */
export const useThemeStore = defineStore('theme', () => {
  const theme = ref<ThemeMode>('dark')

  function apply(t: ThemeMode) {
    theme.value = t
    document.documentElement.dataset.theme = t
    document.documentElement.style.colorScheme = t
  }

  /** 启动时初始化：localStorage → 系统偏好 */
  function init() {
    const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null
    apply(saved === 'light' || saved === 'dark' ? saved : systemTheme())
  }

  function toggle() {
    apply(theme.value === 'dark' ? 'light' : 'dark')
    localStorage.setItem(STORAGE_KEY, theme.value)
  }

  return { theme, init, toggle }
})
