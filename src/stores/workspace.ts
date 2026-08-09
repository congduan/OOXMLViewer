import { computed, ref, shallowRef } from 'vue'
import { defineStore } from 'pinia'
import {
  openOoxml,
  readEntry,
  readImage,
  imageDataUrl,
  saveChanges,
  listBackups,
  restoreBackup,
  isTextualPath,
  isImagePath,
  type BackupInfo,
} from '../lib/backend'
import { buildTree } from '../lib/tree'
import type { OpenResult, TreeNode, ZipEntry } from '../types'

interface CurrentEntry {
  entry: ZipEntry
  kind: 'text' | 'binary' | 'image'
  /** text: 文本内容；image: data URL */
  original: string
  content: string
  previewTruncated: boolean
}

/** 由新增条目构造虚拟 ZipEntry（大小按 UTF-8 字节数估算） */
function virtualEntry(path: string, content: string): ZipEntry {
  const name = path.split('/').filter(Boolean).pop() ?? path
  return {
    path,
    name,
    display_name: null,
    is_dir: false,
    size: new TextEncoder().encode(content).length,
    compressed_size: 0,
  }
}

export const useWorkspaceStore = defineStore('workspace', () => {
  const file = shallowRef<OpenResult | null>(null)
  // 深度响应式：updateContent 修改内部 content 属性时需触发 dirty 等计算属性更新
  const current = ref<CurrentEntry | null>(null)
  const loading = ref(false)
  const error = ref('')
  const toast = ref('')
  const selectedPath = ref('')

  // 待保存变更：新增条目（路径→内容）与待删除条目，未点“保存”前不写盘
  const pendingAdds = ref(new Map<string, string>())
  const pendingDeletes = ref(new Set<string>())

  // 最近一次保存时新增的条目（已写入 OOXML，仍可单独“撤销添加”）
  const lastSavedAdds = ref<string[]>([])

  // 该文件保存时自动生成的备份（保存后可回退删除/修改）
  const backups = ref<BackupInfo[]>([])

  let toastTimer: ReturnType<typeof setTimeout> | null = null

  // 磁盘上的原始条目索引（不含待新增/待删除）
  const diskEntryMap = new Map<string, ZipEntry>()

  /** 当前生效的条目清单 = 磁盘条目（含软删除标记的）+ 待新增（同名覆盖磁盘条目） */
  const effectiveEntries = computed<ZipEntry[]>(() => {
    if (!file.value) return []
    const added = pendingAdds.value
    const out = file.value.entries.filter((e) => !added.has(e.path))
    for (const [path, content] of added) {
      out.push(virtualEntry(path, content))
    }
    return out
  })

  const tree = computed<TreeNode[]>(() =>
    buildTree(effectiveEntries.value, pendingDeletes.value),
  )

  const dirty = computed(
    () =>
      pendingAdds.value.size > 0 ||
      pendingDeletes.value.size > 0 ||
      (!!current.value && current.value.content !== current.value.original),
  )
  const currentEntry = computed(() => current.value?.entry ?? null)
  const currentEditable = computed(() => !!current.value && current.value.kind === 'text')
  const totalEntries = computed(() => effectiveEntries.value.length)
  const hasBackup = computed(() => backups.value.length > 0)

  function showToast(msg: string) {
    toast.value = msg
    if (toastTimer) clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast.value = ''), 3000)
  }

  function setError(msg: string) {
    error.value = msg
  }

  /** 刷新当前文件的备份列表（失败时静默清空） */
  async function refreshBackups() {
    if (!file.value) {
      backups.value = []
      return
    }
    try {
      backups.value = await listBackups(file.value.file_path)
    } catch {
      backups.value = []
    }
  }

  async function openFile(path: string) {
    if (dirty.value) {
      const ok = window.confirm('You have unsaved changes. Opening a new file will discard them. Continue?')
      if (!ok) return
    }
    loading.value = true
    error.value = ''
    try {
      const result = await openOoxml(path)
      file.value = result
      diskEntryMap.clear()
      for (const e of result.entries) diskEntryMap.set(e.path, e)
      pendingAdds.value.clear()
      pendingDeletes.value.clear()
      lastSavedAdds.value = []
      current.value = null
      selectedPath.value = ''
      await refreshBackups()
      showToast(`Opened ${result.file_name}`)
    } catch (e) {
      setError(String(e))
    } finally {
      loading.value = false
    }
  }

  async function selectEntry(entry: ZipEntry) {
    if (!file.value) return
    // 同一条目直接忽略（脏状态由编辑器保留）
    if (current.value?.entry.path === entry.path) return
    if (dirty.value) {
      const ok = window.confirm('This entry has unsaved changes. Switching will discard them. Continue?')
      if (!ok) return
    }
    loading.value = true
    error.value = ''
    try {
      const added = pendingAdds.value.get(entry.path)
      if (added !== undefined) {
        // 待新增条目：内容直接来自内存
        current.value = {
          entry,
          kind: 'text',
          original: added,
          content: added,
          previewTruncated: false,
        }
      } else if (isImagePath(entry.path)) {
        // 图片条目：读取 base64 并以 data URL 预览
        const b64 = await readImage(file.value.file_path, entry.path)
        const url = imageDataUrl(entry.path, b64)
        current.value = {
          entry,
          kind: 'image',
          original: url,
          content: url,
          previewTruncated: false,
        }
      } else {
        const result = await readEntry(file.value.file_path, entry.path)
        current.value = {
          entry,
          kind: result.kind,
          original: result.content,
          content: result.content,
          previewTruncated: result.preview_truncated,
        }
      }
      selectedPath.value = entry.path
    } catch (e) {
      setError(String(e))
    } finally {
      loading.value = false
    }
  }

  function updateContent(content: string) {
    if (current.value) current.value.content = content
  }

  async function save() {
    if (!file.value || !dirty.value) return
    loading.value = true
    error.value = ''
    try {
      const adds: { path: string; content: string }[] = []
      for (const [path, content] of pendingAdds.value) {
        // 该新增条目正被编辑时，使用编辑器里的最新内容，避免编辑丢失
        const live = current.value?.entry.path === path ? current.value.content : content
        adds.push({ path, content: live })
      }
      const updates: { path: string; content: string }[] = []
      if (
        current.value?.kind === 'text' &&
        current.value.content !== current.value.original &&
        !pendingAdds.value.has(current.value.entry.path)
      ) {
        updates.push({ path: current.value.entry.path, content: current.value.content })
      }
      const deletes = [...pendingDeletes.value]
      // 记录本次保存将新增的条目（保存后仍可单独“撤销添加”）
      const savedAdds = [...pendingAdds.value.keys()]
      await saveChanges(file.value.file_path, adds, updates, deletes)
      pendingAdds.value.clear()
      pendingDeletes.value.clear()
      lastSavedAdds.value = savedAdds
      await reloadEntries()
      await refreshBackups()
      showToast('Saved (use "Restore" to revert deletes/edits)')
    } catch (e) {
      setError(String(e))
    } finally {
      loading.value = false
    }
  }

  /** 保存/恢复后重新读取磁盘清单，尽量保持当前选中条目。
   *  reloadContent=true 时（恢复场景）连当前条目内容也从磁盘重新读取。 */
  async function reloadEntries(reloadContent = false) {
    if (!file.value) return
    const prevPath = current.value?.entry.path
    try {
      const result = await openOoxml(file.value.file_path)
      file.value = result
      diskEntryMap.clear()
      for (const e of result.entries) diskEntryMap.set(e.path, e)
      if (prevPath && diskEntryMap.has(prevPath)) {
        if (current.value) {
          current.value.entry = diskEntryMap.get(prevPath)!
          if (reloadContent) {
            if (isImagePath(prevPath)) {
              const b64 = await readImage(file.value.file_path, prevPath)
              const url = imageDataUrl(prevPath, b64)
              current.value.kind = 'image'
              current.value.original = url
              current.value.content = url
              current.value.previewTruncated = false
            } else {
              const r = await readEntry(file.value.file_path, prevPath)
              current.value.kind = r.kind
              current.value.original = r.content
              current.value.content = r.content
              current.value.previewTruncated = r.preview_truncated
            }
          } else {
            // 已写盘，磁盘内容与编辑器一致，同步 original 使 dirty 复位
            current.value.original = current.value.content
          }
        }
        selectedPath.value = prevPath
      } else if (prevPath) {
        current.value = null
        selectedPath.value = ''
      }
    } catch (e) {
      setError(String(e))
    }
  }

  /** 恢复上一次保存的版本（回退删除/修改），恢复后整份文件回到上次保存时的内容 */
  async function restoreLastSave() {
    if (!file.value || backups.value.length === 0) {
      if (file.value) showToast('No backup available to restore')
      return
    }
    if (dirty.value) {
      const ok = window.confirm('You have unsaved changes. Restoring will discard them. Continue?')
      if (!ok) return
    }
    loading.value = true
    error.value = ''
    try {
      const latest = backups.value[0] // 已按修改时间倒序
      await restoreBackup(file.value.file_path, latest.backup_path)
      lastSavedAdds.value = []
      await reloadEntries(true)
      await refreshBackups()
      showToast('Restored to the last saved version')
    } catch (e) {
      setError(String(e))
    } finally {
      loading.value = false
    }
  }

  /** 新增文本条目：仅加入待保存集合，点“保存”后才写盘 */
  function addEntry(entryPath: string, content: string) {
    if (!file.value) return
    const p = entryPath.trim()
    if (!p || p.endsWith('/')) {
      setError('Enter a valid entry path')
      return
    }
    if (!isTextualPath(p)) {
      setError('Only text files can be added (.xml / .rels / .json, etc.)')
      return
    }
    if (diskEntryMap.has(p) && !pendingDeletes.value.has(p)) {
      setError(`Entry [${p}] already exists`)
      return
    }
    if (pendingAdds.value.has(p)) {
      setError(`Entry [${p}] is already added (pending save)`)
      return
    }
    pendingDeletes.value.delete(p)
    pendingAdds.value.set(p, content)
    current.value = {
      entry: virtualEntry(p, content),
      kind: 'text',
      original: content,
      content,
      previewTruncated: false,
    }
    selectedPath.value = p
    showToast(`Added ${p} (pending save)`)
  }

  /** 删除条目：标记为软删除（树中删除线显示），点“保存”后才写盘，保存前可撤销 */
  function removeEntry(entryPath: string) {
    if (!file.value) return
    // 待新增条目：直接撤销添加，无需进入删除集合
    if (pendingAdds.value.has(entryPath)) {
      restoreEntry(entryPath)
      return
    }
    pendingDeletes.value.add(entryPath)
    pendingAdds.value.delete(entryPath)
    if (current.value?.entry.path === entryPath) {
      current.value = null
      selectedPath.value = ''
    }
    showToast(`Deleted ${entryPath} (pending save, undoable)`)
  }

  /** 撤销删除 / 撤销添加：恢复条目。
   *  待保存新增（pendingAdds）直接取消；已保存的新增（lastSavedAdds）转为软删除，保存后从文件中移除 */
  function restoreEntry(entryPath: string) {
    if (!file.value) return
    if (pendingDeletes.value.delete(entryPath)) {
      showToast(`Undid delete of ${entryPath}`)
      return
    }
    if (pendingAdds.value.delete(entryPath)) {
      // 该新增条目正被编辑时一并关闭
      if (current.value?.entry.path === entryPath) {
        current.value = null
        selectedPath.value = ''
      }
      showToast(`Undid add of ${entryPath}`)
      return
    }
    if (lastSavedAdds.value.includes(entryPath)) {
      // 已写入 OOXML 的新增文件：标记软删除，保存后从文件中移除
      pendingDeletes.value.add(entryPath)
      if (current.value?.entry.path === entryPath) {
        current.value = null
        selectedPath.value = ''
      }
      showToast(`Undid add of ${entryPath} (removed after save)`)
    }
  }

  function closeFile() {
    file.value = null
    current.value = null
    selectedPath.value = ''
    pendingAdds.value.clear()
    pendingDeletes.value.clear()
    lastSavedAdds.value = []
    diskEntryMap.clear()
    backups.value = []
    error.value = ''
  }

  return {
    file,
    tree,
    current,
    loading,
    error,
    toast,
    selectedPath,
    dirty,
    currentEntry,
    currentEditable,
    totalEntries,
    backups,
    hasBackup,
    pendingAdds,
    lastSavedAdds,
    openFile,
    selectEntry,
    updateContent,
    save,
    restoreLastSave,
    addEntry,
    removeEntry,
    restoreEntry,
    closeFile,
    showToast,
    setError,
  }
})
