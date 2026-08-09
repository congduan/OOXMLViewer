<script setup lang="ts">
import { computed, inject } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import { formatSize, isImagePath, isTextualPath } from '../lib/backend'
import type { TreeNode } from '../types'

defineOptions({ name: 'EntryTreeNode' })

const props = defineProps<{
  node: TreeNode
  depth: number
  expanded: Set<string>
  forceExpand: boolean
}>()

const store = useWorkspaceStore()

/** 由父级 EntryTree 注入的右键菜单打开函数 */
const openMenu = inject<(e: MouseEvent, node: TreeNode) => void>('tree-open-menu')!

const isDir = computed(() => props.node.type === 'dir')
const isOpen = computed(() => props.forceExpand || props.expanded.has(props.node.path))
const isSelected = computed(() => store.selectedPath === props.node.path)
/** 仅当该文件自身内容被修改且未保存时显示指示器（与其他文件的增删变更无关） */
const isDirty = computed(
  () =>
    store.current?.entry.path === props.node.path &&
    !!store.current &&
    store.current.content !== store.current.original,
)
const isImage = computed(() => isImagePath(props.node.path))
const isBinary = computed(() => !isTextualPath(props.node.path) && !isImage.value)
const isPendingDelete = computed(() => !!props.node.pendingDelete)
/** 待保存的新增文件（可撤销添加） */
const isAdded = computed(() => store.pendingAdds.has(props.node.path))
/** 最近一次保存新增、已写入 OOXML 的文件（仍可撤销添加） */
const isRecentAdd = computed(() => store.lastSavedAdds.includes(props.node.path))
/** 展示“新增”标记：待保存新增或最近保存新增，且未被标记删除 */
const isMarkedAdded = computed(() => (isAdded.value || isRecentAdd.value) && !isPendingDelete.value)

function onRowClick() {
  if (isDir.value) {
    if (props.forceExpand) return
    if (props.expanded.has(props.node.path)) props.expanded.delete(props.node.path)
    else props.expanded.add(props.node.path)
    return
  }
  // 软删除条目：点击即撤销删除并打开
  if (isPendingDelete.value) {
    store.restoreEntry(props.node.path)
  }
  void store.selectEntry({
    path: props.node.path,
    name: props.node.name,
    display_name: props.node.displayName ?? null,
    is_dir: false,
    size: props.node.size ?? 0,
    compressed_size: props.node.compressedSize ?? 0,
  })
}

function onRestore() {
  store.restoreEntry(props.node.path)
}

/** 撤销新增（待保存或最近保存已写入的新增文件） */
function onUndoAdd() {
  store.restoreEntry(props.node.path)
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  e.stopPropagation()
  openMenu(e, props.node)
}
</script>

<template>
  <li class="tree-node">
    <div
      class="tree-row"
      :class="{
        selected: isSelected,
        dir: isDir,
        binary: isBinary,
        dirty: isDirty,
        deleting: isPendingDelete,
        added: isMarkedAdded,
      }"
      :style="{ paddingLeft: `${6 + depth * 14}px` }"
      :data-path="node.path"
      @click="onRowClick"
      @contextmenu="onContextMenu"
    >
      <span class="arrow" :class="{ open: isDir && isOpen }">
        <svg v-if="isDir" viewBox="0 0 16 16" width="12" height="12">
          <path d="M6 4l4 4-4 4z" fill="currentColor" />
        </svg>
        <svg v-else viewBox="0 0 16 16" width="12" height="12" class="file-dot">
          <circle cx="8" cy="8" r="3" fill="currentColor" />
        </svg>
      </span>
      <span class="label" :title="node.path">
        <template v-if="node.displayName && node.displayName !== node.name">
          <span class="label-primary">{{ node.displayName }}</span>
          <span class="label-sub">({{ node.name }})</span>
        </template>
        <template v-else>{{ node.name }}</template>
      </span>
      <span v-if="!isDir" class="meta">
        <span v-if="isDirty" class="dirty-dot" title="Unsaved"></span>
        <button
          v-if="isPendingDelete"
          class="restore-btn"
          title="Undo delete"
          @click.stop="onRestore"
        >
          <svg viewBox="0 0 16 16" width="12" height="12">
            <path
              d="M6 4L2.5 7.5 6 11M3 7.5h6a4.5 4.5 0 0 1 0 9H7"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </button>
        <button
          v-if="isMarkedAdded"
          class="restore-btn"
          :title="isRecentAdd ? 'Undo add (removed after save)' : 'Undo add'"
          @click.stop="onUndoAdd"
        >
          <svg viewBox="0 0 16 16" width="12" height="12">
            <path
              d="M3 4.5v3.5h3.5M3 8a5 5 0 1 0 2.1-3.8M8 5.5v3l1.8 1"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </button>
        <span v-if="isMarkedAdded" class="badge added-badge">new</span>
        <span v-if="isImage" class="badge">img</span>
        <span v-if="isBinary" class="badge">bin</span>
        <span class="size">{{ formatSize(node.size ?? 0) }}</span>
      </span>
    </div>
    <ul v-if="isDir && isOpen" class="tree-children">
      <EntryTreeNode
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :depth="depth + 1"
        :expanded="expanded"
        :force-expand="forceExpand"
      />
    </ul>
  </li>
</template>

<style scoped>
.tree-node {
  list-style: none;
}

.tree-row {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding-right: 8px;
  cursor: default;
  color: var(--fg);
  white-space: nowrap;
}

.tree-row:hover {
  background: var(--hover);
}

.tree-row.selected {
  background: var(--accent-dim);
  color: var(--accent);
}

.tree-row.binary .label {
  color: var(--fg-dim);
}

/* 软删除态 */
.tree-row.deleting .label {
  text-decoration: line-through;
  color: var(--fg-dim);
  opacity: 0.7;
}

/* 待保存新增态 */
.tree-row.added .label {
  color: #8fd18f;
}

.added-badge {
  color: #8fd18f;
  border-color: rgba(143, 209, 143, 0.45);
}

.restore-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--fg-dim);
  cursor: pointer;
  flex: none;
}

.restore-btn:hover {
  background: var(--accent-dim);
  color: var(--accent);
}

.arrow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  flex: none;
  color: var(--fg-dim);
  transition: transform 0.12s;
}

.arrow.open {
  transform: rotate(90deg);
}

.arrow .file-dot {
  color: var(--fg-dim);
}

.tree-row.selected .arrow {
  color: var(--accent);
}

.label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 12.5px;
}

.label-sub {
  color: var(--fg-dim);
  font-size: 11px;
  margin-left: 2px;
}

.meta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex: none;
}

.dirty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #e5c07b;
  flex: none;
}

.badge {
  font-size: 9px;
  color: var(--fg-dim);
  border: 1px solid var(--border);
  border-radius: 3px;
  padding: 0 3px;
  line-height: 13px;
}

.size {
  font-size: 10.5px;
  color: var(--fg-dim);
}

.tree-children {
  margin: 0;
  padding: 0;
}
</style>
