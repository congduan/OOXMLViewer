import type { TreeNode, ZipEntry } from '../types'

/** 由 zip 扁平条目列表构建目录树；pendingDeletes 中的文件标记为软删除态 */
export function buildTree(
  entries: ZipEntry[],
  pendingDeletes: ReadonlySet<string> = new Set(),
): TreeNode[] {
  const root: TreeNode[] = []
  const index = new Map<string, TreeNode>()

  for (const e of entries) {
    if (e.is_dir) continue // 跳过显式的目录条目，纯按路径分段建树
    const parts = e.path.split('/').filter(Boolean)
    if (parts.length === 0) continue
    let nodes = root
    let acc = ''
    for (let idx = 0; idx < parts.length; idx++) {
      const part = parts[idx]
      acc = acc ? `${acc}/${part}` : part
      const isLast = idx === parts.length - 1
      let node = index.get(acc)
      if (!node) {
        node = {
          path: acc,
          name: part,
          displayName: isLast ? e.display_name ?? undefined : undefined,
          pendingDelete: isLast ? pendingDeletes.has(e.path) : undefined,
          type: isLast ? 'file' : 'dir',
          children: [],
          size: isLast ? e.size : undefined,
          compressedSize: isLast ? e.compressed_size : undefined,
        }
        index.set(acc, node)
        nodes.push(node)
      }
      if (!isLast) nodes = node.children
    }
  }

  const sortNodes = (ns: TreeNode[]) => {
    ns.sort((a, b) => {
      if (a.type !== b.type) return a.type === 'dir' ? -1 : 1
      return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
    })
    ns.forEach((n) => sortNodes(n.children))
  }
  sortNodes(root)
  return root
}

/** 按关键字过滤树：保留路径命中关键字的文件及其祖先目录 */
export function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  const q = query.trim().toLowerCase()
  if (!q) return nodes
  const result: TreeNode[] = []
  for (const n of nodes) {
    if (n.type === 'dir') {
      const children = filterTree(n.children, q)
      if (children.length) result.push({ ...n, children })
    } else if (n.name.toLowerCase().includes(q) || n.path.toLowerCase().includes(q)) {
      result.push(n)
    }
  }
  return result
}
