import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import type { OpenResult, ReadEntryResult } from '../types'

/** 打开 OOXML 文件，返回内部条目清单 */
export function openOoxml(filePath: string): Promise<OpenResult> {
  return invoke<OpenResult>('open_ooxml', { filePath })
}

/** 读取指定条目内容 */
export function readEntry(filePath: string, entryPath: string): Promise<ReadEntryResult> {
  return invoke<ReadEntryResult>('read_entry', { filePath, entryPath })
}

/** 读取 zip 内图片条目，返回 base64 编码内容（用于 data URL 预览） */
export function readImage(filePath: string, entryPath: string): Promise<string> {
  return invoke<string>('read_image', { filePath, entryPath })
}

/** 读取整个 OOXML 文件，返回 base64 编码内容（用于整文件预览渲染） */
export function readWholeFile(filePath: string): Promise<string> {
  return invoke<string>('read_whole_file', { filePath })
}

/** 单条条目变更（新增/更新共用） */
export interface EntryChange {
  path: string
  content: string
}

/** 一次性应用增/删/改变更并原子写回文件，返回写入字节数 */
export function saveChanges(
  filePath: string,
  adds: EntryChange[],
  updates: EntryChange[],
  deletes: string[],
): Promise<number> {
  return invoke<number>('save_changes', { filePath, adds, updates, deletes })
}

/** 备份信息（保存时自动生成，用于“恢复上一次保存”） */
export interface BackupInfo {
  backup_path: string
  file_name: string
  size: number
  modified_ms: number
}

/** 列出当前文件的备份（按修改时间倒序） */
export function listBackups(filePath: string): Promise<BackupInfo[]> {
  return invoke<BackupInfo[]>('list_backups', { filePath })
}

/** 用指定备份覆盖当前文件（删除/修改错了可回退到上一次保存），返回写入字节数 */
export function restoreBackup(filePath: string, backupPath: string): Promise<number> {
  return invoke<number>('restore_backup', { filePath, backupPath })
}

/** 提取条目到目标目录（保持相对路径），返回写入字节数 */
export function extractEntry(
  filePath: string,
  entryPath: string,
  destDir: string,
): Promise<number> {
  return invoke<number>('extract_entry', { filePath, entryPath, destDir })
}

/** 选择导出目标目录 */
export async function pickDirectory(title: string): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title })
  return typeof selected === 'string' ? selected : null
}

/** 通过系统对话框选择 OOXML 文件 */
export async function pickFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    title: 'Choose an OOXML file',
    filters: [
      { name: 'OOXML documents', extensions: ['docx', 'docm', 'xlsx', 'xlsm', 'pptx', 'pptm'] },
      { name: 'All files', extensions: ['*'] },
    ],
  })
  return typeof selected === 'string' ? selected : null
}

/** 在系统文件管理器中显示该文件（打开所在文件夹并定位） */
export function revealInFolder(filePath: string): Promise<void> {
  return revealItemInDir(filePath)
}

/** 订阅系统级拖拽事件（返回取消订阅函数） */
export async function onFileDrop(handlers: {
  over?: () => void
  leave?: () => void
  drop?: (paths: string[]) => void
}): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload
    if (p.type === 'enter' || p.type === 'over') handlers.over?.()
    else if (p.type === 'leave') handlers.leave?.()
    else if (p.type === 'drop') handlers.drop?.(p.paths)
  })
}

/** 判定 zip 条目是否属于可编辑的文本文件（与 Rust 端规则保持一致） */
const TEXTUAL_EXTS = [
  '.xml', '.rels', '.json', '.txt', '.csv', '.tsv', '.js', '.css', '.html', '.htm',
  '.md', '.properties', '.dtd', '.xsd', '.xsl', '.yml', '.yaml', '.rng',
]

export function isTextualPath(path: string): boolean {
  if (path.endsWith('/')) return false
  const lower = path.toLowerCase()
  return TEXTUAL_EXTS.some((ext) => lower.endsWith(ext))
}

/** 判定 zip 条目是否为可预览的图片文件（svg 按图片渲染，与 Rust 端规则一致） */
const IMAGE_EXTS = ['.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp', '.ico', '.tif', '.tiff', '.svg']

export function isImagePath(path: string): boolean {
  const lower = path.toLowerCase()
  return IMAGE_EXTS.some((ext) => lower.endsWith(ext))
}

/** 图片扩展名 → data URL mime */
const IMAGE_MIME: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.bmp': 'image/bmp',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.tif': 'image/tiff',
  '.tiff': 'image/tiff',
  '.svg': 'image/svg+xml',
}

/** 由条目路径生成图片 data URL（如 data:image/png;base64,...） */
export function imageDataUrl(path: string, base64: string): string {
  const lower = path.toLowerCase()
  const mime = IMAGE_EXTS.find((ext) => lower.endsWith(ext))
  return `data:${IMAGE_MIME[mime ?? '.png'] ?? 'image/png'};base64,${base64}`
}

/** 根据扩展名推断 Monaco language id */
export function languageForPath(path: string): string {
  const lower = path.toLowerCase()
  if (lower.endsWith('.xml') || lower.endsWith('.rels')) return 'xml'
  if (lower.endsWith('.json')) return 'json'
  if (lower.endsWith('.js')) return 'javascript'
  if (lower.endsWith('.css')) return 'css'
  if (lower.endsWith('.html') || lower.endsWith('.htm') || lower.endsWith('.svg')) return 'html'
  if (lower.endsWith('.md')) return 'markdown'
  if (lower.endsWith('.yml') || lower.endsWith('.yaml')) return 'yaml'
  if (lower.endsWith('.properties')) return 'properties'
  return 'plaintext'
}

/** 人类可读的文件大小 */
export function formatSize(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const v = bytes / 1024 ** i
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`
}
