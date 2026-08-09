export interface ZipEntry {
  /** zip 内完整路径，如 "word/document.xml" */
  path: string
  name: string
  /** 显示名（如 xlsx 工作表名称 "Sheet1"），无特殊显示名时为 null */
  display_name: string | null
  is_dir: boolean
  size: number
  compressed_size: number
}

export interface OpenResult {
  file_name: string
  file_path: string
  file_size: number
  kind: 'docx' | 'xlsx' | 'pptx' | 'ooxml'
  entries: ZipEntry[]
}

export interface ReadEntryResult {
  kind: 'text' | 'binary'
  content: string
  size: number
  preview_truncated: boolean
}

export interface TreeNode {
  path: string
  name: string
  /** 显示名（如 xlsx 工作表名称），无则回退到 name */
  displayName?: string
  /** 该文件已被标记待删除（软删除，可撤销） */
  pendingDelete?: boolean
  type: 'dir' | 'file'
  children: TreeNode[]
  size?: number
  compressedSize?: number
}
