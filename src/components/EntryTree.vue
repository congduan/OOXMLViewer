<script setup lang="ts">
import { computed, nextTick, onMounted, provide, reactive, ref, watch } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import EntryTreeNode from './EntryTreeNode.vue'
import ContextMenu from './ContextMenu.vue'
import { filterTree } from '../lib/tree'
import type { TreeNode } from '../types'

const store = useWorkspaceStore()

const emit = defineEmits<{
  (e: 'add-file', parentDir: string): void
  (e: 'delete-file', path: string): void
  (e: 'restore-file', path: string): void
  (e: 'extract-file', path: string): void
}>()

const query = ref('')
const expanded = reactive(new Set<string>())
const filtering = computed(() => query.value.trim() !== '')
const visibleTree = computed(() => filterTree(store.tree, query.value))

const menuState = ref<{ x: number; y: number; node: TreeNode } | null>(null)

// 打开文件后默认展开第一层目录（word/ppt/xl/docProps 等）
onMounted(() => {
  for (const node of store.tree) {
    if (node.type === 'dir') expanded.add(node.path)
  }
})

// 选中条目变化时，自动展开其所有祖先目录并滚动到可见位置。
// 修复：新增文件位于未展开（或不存在的）父目录下时列表看不到
watch(
  () => store.selectedPath,
  async (path) => {
    if (!path) return
    const parts = path.split('/')
    let acc = ''
    for (let i = 0; i < parts.length - 1; i++) {
      acc = acc ? `${acc}/${parts[i]}` : parts[i]
      expanded.add(acc)
    }
    await nextTick()
    const el = document.querySelector(
      `.tree-row[data-path="${CSS.escape(path)}"]`,
    )
    el?.scrollIntoView({ block: 'nearest' })
  },
)

// 供递归子节点触发的右键菜单
provide('tree-open-menu', (e: MouseEvent, node: TreeNode) => {
  const W = 200
  const H = 120
  menuState.value = {
    x: Math.min(e.clientX, window.innerWidth - W - 8),
    y: Math.min(e.clientY, window.innerHeight - H - 8),
    node,
  }
})

function parentDirOf(node: TreeNode): string {
  if (node.type === 'dir') return node.path
  const idx = node.path.lastIndexOf('/')
  return idx > 0 ? node.path.slice(0, idx) : ''
}

function onMenuAdd() {
  const node = menuState.value?.node
  if (!node) return
  emit('add-file', parentDirOf(node))
  menuState.value = null
}

function onMenuDelete() {
  const node = menuState.value?.node
  if (!node) return
  emit('delete-file', node.path)
  menuState.value = null
}

function onMenuCopy() {
  const node = menuState.value?.node
  if (!node) return
  void navigator.clipboard.writeText(node.path)
  menuState.value = null
}

function onMenuExtract() {
  const node = menuState.value?.node
  if (!node) return
  emit('extract-file', node.path)
  menuState.value = null
}

function onMenuRestore() {
  const node = menuState.value?.node
  if (!node) return
  emit('restore-file', node.path)
  menuState.value = null
}
</script>

<template>
  <div class="entry-tree">
    <div class="tree-toolbar">
      <input
        v-model="query"
        type="text"
        class="tree-filter"
        placeholder="Filter files..."
        spellcheck="false"
      />
      <button
        class="add-btn"
        title="Add file (root)"
        @click="emit('add-file', '')"
      >
        <svg viewBox="0 0 16 16" width="13" height="13">
          <path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>
      <span class="tree-count">{{ store.totalEntries }}</span>
    </div>
    <div class="tree-scroll">
      <ul v-if="visibleTree.length" class="tree-root">
        <EntryTreeNode
          v-for="node in visibleTree"
          :key="node.path"
          :node="node"
          :depth="0"
          :expanded="expanded"
          :force-expand="filtering"
        />
      </ul>
      <div v-else-if="filtering" class="tree-empty">No matching entries</div>
      <div v-else class="tree-empty">(empty)</div>
    </div>

    <ContextMenu
      v-if="menuState"
      :x="menuState.x"
      :y="menuState.y"
      :is-dir="menuState.node.type === 'dir'"
      :pending-delete="!!menuState.node.pendingDelete"
      :pending-add="
        store.pendingAdds.has(menuState.node.path) ||
        store.lastSavedAdds.includes(menuState.node.path)
      "
      @close="menuState = null"
      @add="onMenuAdd"
      @delete="onMenuDelete"
      @restore="onMenuRestore"
      @copy="onMenuCopy"
      @extract="onMenuExtract"
    />
  </div>
</template>

<style scoped>
.entry-tree {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
}

.tree-toolbar {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  gap: 8px;
}

.tree-filter {
  flex: 1;
  min-width: 0;
  height: 26px;
  padding: 0 8px;
  border-radius: 5px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--fg);
  font-size: 12px;
  outline: none;
}

.tree-filter:focus {
  border-color: var(--accent);
}

.add-btn {
  flex: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 5px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--fg);
  cursor: pointer;
}

.add-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.tree-count {
  font-size: 11px;
  color: var(--fg-dim);
  flex: none;
}

.tree-scroll {
  flex: 1;
  overflow: auto;
  padding: 6px 0;
}

.tree-root {
  margin: 0;
  padding: 0;
}

.tree-empty {
  padding: 24px 12px;
  text-align: center;
  color: var(--fg-dim);
  font-size: 12px;
}
</style>
