<template>
  <el-container class="file-browser">
    <el-header class="toolbar">
      <div class="toolbar-left">
        <el-button class="menu-toggle" link @click="sidebarOpen = !sidebarOpen">
          <el-icon :size="20"><Menu /></el-icon>
        </el-button>
        <el-breadcrumb separator="/">
          <el-breadcrumb-item v-for="item in breadcrumbs" :key="item.path">
            <a @click="navigateTo(item.path)">{{ item.name }}</a>
          </el-breadcrumb-item>
        </el-breadcrumb>
      </div>
      <div class="toolbar-right">
        <el-input v-model="searchKeyword" placeholder="搜索文件..." class="search-input" clearable @keyup.enter="doSearch">
          <template #append>
            <el-button @click="doSearch">搜索</el-button>
          </template>
        </el-input>
        <el-button-group>
          <el-button :type="viewMode === 'list' ? 'primary' : ''" @click="viewMode = 'list'">
            <el-icon><List /></el-icon>
          </el-button>
          <el-button :type="viewMode === 'grid' ? 'primary' : ''" @click="viewMode = 'grid'">
            <el-icon><Grid /></el-icon>
          </el-button>
        </el-button-group>
        <el-button type="primary" @click="showUploadDialog = true">上传</el-button>
        <el-button @click="showMkdirDialog = true">新建文件夹</el-button>
        <el-dropdown v-if="authStore.isAdmin()">
          <el-button>管理<el-icon><ArrowDown /></el-icon></el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item @click="$router.push('/admin/users')">用户管理</el-dropdown-item>
              <el-dropdown-item @click="$router.push('/admin/permissions')">权限管理</el-dropdown-item>
              <el-dropdown-item @click="$router.push('/admin/audit')">审计日志</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button @click="handleLogout">退出</el-button>
      </div>
    </el-header>
    <el-container class="main-container">
      <div v-if="isMobile && sidebarOpen" class="sidebar-mask" @click="sidebarOpen = false" />
      <el-aside class="sidebar" :class="{ 'sidebar-open': isMobile && sidebarOpen }">
        <div class="sidebar-header">
          <span class="sidebar-title">目录</span>
          <el-button v-if="isMobile" link @click="sidebarOpen = false" class="sidebar-close">
            <el-icon><Close /></el-icon>
          </el-button>
        </div>
        <el-tree :data="treeData" :props="treeProps" lazy :load="loadTreeNode" @node-click="onTreeNodeClick" highlight-current node-key="path" :default-expanded-keys="['/']" />
      </el-aside>
      <el-main class="file-main">
        <el-table v-if="viewMode === 'list'" :data="fileList" @row-dblclick="onRowDblClick" @row-contextmenu="onRowContextMenu" stripe>
          <el-table-column prop="name" label="名称" min-width="300">
            <template #default="{ row }">
              <el-icon v-if="row.is_dir" style="color: #409eff"><Folder /></el-icon>
              <el-icon v-else><Document /></el-icon>
              <span style="margin-left: 8px; cursor: pointer">{{ row.name }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="size" label="大小" width="120">
            <template #default="{ row }">{{ row.is_dir ? '-' : formatSize(row.size) }}</template>
          </el-table-column>
          <el-table-column prop="modified" label="修改时间" width="180" />
          <el-table-column label="操作" width="360">
            <template #default="{ row }">
              <el-button size="small" @click="viewOnlyOffice(row)" v-if="!row.is_dir && canEditOffice(row)">查看</el-button>
              <el-button size="small" type="primary" @click="openOnlyOfficeEdit(row)" v-if="!row.is_dir && canEditOffice(row)">编辑</el-button>
              <el-button size="small" @click="previewFile(row)" v-if="!row.is_dir && canPreview(row)">预览</el-button>
              <el-button size="small" @click="downloadFile(row)" v-if="!row.is_dir">下载</el-button>
              <el-button size="small" @click="showRenameDialog(row)">重命名</el-button>
              <el-button size="small" type="danger" @click="confirmDelete(row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
        <div v-else class="grid-view">
          <div v-for="file in fileList" :key="file.path" class="grid-card" @dblclick="onRowDblClick(file)" @contextmenu.prevent="onRowContextMenu(file, $event)">
            <div class="card-icon" :style="{ background: getFileIconColor(file) }">
              <el-icon v-if="file.is_dir" :size="28" color="#fff"><Folder /></el-icon>
              <span v-else class="card-icon-text">{{ getFileIconLabel(file) }}</span>
            </div>
            <div class="card-info">
              <div class="card-name" :title="file.name">{{ file.name }}</div>
              <div class="card-meta">
                <span v-if="!file.is_dir">{{ formatSize(file.size) }}</span>
                <span v-else>文件夹</span>
                <span class="card-date">{{ formatDate(file.modified) }}</span>
              </div>
            </div>
            <div class="card-actions">
              <el-button v-if="!file.is_dir && canEditOffice(file)" size="small" circle @click.stop="viewOnlyOffice(file)" title="查看">
                <el-icon><View /></el-icon>
              </el-button>
              <el-button v-if="!file.is_dir && canPreview(file) && !canEditOffice(file)" size="small" circle @click.stop="previewFile(file)" title="预览">
                <el-icon><View /></el-icon>
              </el-button>
              <el-button v-if="!file.is_dir && canEditOffice(file)" size="small" circle type="primary" @click.stop="openOnlyOfficeEdit(file)" title="编辑">
                <el-icon><Edit /></el-icon>
              </el-button>
              <el-button v-if="!file.is_dir" size="small" circle @click.stop="downloadFile(file)" title="下载">
                <el-icon><Download /></el-icon>
              </el-button>
            </div>
          </div>
        </div>
      </el-main>
    </el-container>

    <el-dialog v-model="showMkdirDialog" title="新建文件夹" width="400px">
      <el-input v-model="newFolderName" placeholder="请输入文件夹名称" />
      <template #footer>
        <el-button @click="showMkdirDialog = false">取消</el-button>
        <el-button type="primary" @click="createFolder">确定</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="showRenameDialogVisible" title="重命名" width="400px">
      <el-input v-model="renameNewName" placeholder="请输入新名称" />
      <template #footer>
        <el-button @click="showRenameDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="doRename">确定</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="showUploadDialog" title="上传文件" width="500px">
      <el-upload drag multiple :auto-upload="false" :on-change="onFileChange" :file-list="uploadFileList">
        <el-icon :size="64"><Upload /></el-icon>
        <div>拖拽文件到此处或点击上传</div>
      </el-upload>
      <template #footer>
        <el-button @click="showUploadDialog = false">取消</el-button>
        <el-button type="primary" @click="startUpload" :loading="uploading">上传</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="showPreviewDialog" :title="previewFileName" class="preview-dialog" :width="isMobile ? '100%' : '85%'" :top="isMobile ? '0' : '3vh'" :fullscreen="isMobile">
      <div class="preview-container">
        <img v-if="previewType === 'image'" :src="previewUrl" style="max-width: 100%; max-height: calc(100vh - 180px)" />
        <video v-else-if="previewType === 'video'" :src="previewUrl" controls style="max-width: 100%; max-height: calc(100vh - 180px)" />
        <audio v-else-if="previewType === 'audio'" :src="previewUrl" controls />
        <iframe v-else-if="previewType === 'pdf'" :src="previewUrl" style="width: 100%; height: calc(100vh - 180px); border: none" />
        <pre v-else-if="previewType === 'text'" class="text-preview">{{ textContent }}</pre>
        <div v-else>不支持预览此文件类型</div>
      </div>
    </el-dialog>

    <el-dialog v-model="showOnlyOfficeDialog" :title="`编辑: ${onlyOfficeFileName}`" class="onlyoffice-dialog" :class="{ 'onlyoffice-dialog-fullscreen': isMobile }" :width="isMobile ? '100%' : '95%'" :top="isMobile ? '0' : '2vh'" @close="destroyOnlyOfficeEditor" :close-on-click-modal="false" :fullscreen="isMobile">
      <div v-loading="onlyOfficeLoading" class="onlyoffice-container">
        <div id="onlyoffice-editor" class="onlyoffice-editor"></div>
      </div>
    </el-dialog>

    <el-dialog v-model="showArchiveDialog" :title="`压缩包内容: ${archiveFileName}`" width="700px">
      <el-table :data="archiveEntries" v-loading="archiveLoading" max-height="500" stripe>
        <el-table-column prop="name" label="名称" min-width="250">
          <template #default="{ row }">
            <el-icon v-if="row.is_dir" style="color: #409eff"><Folder /></el-icon>
            <el-icon v-else><Document /></el-icon>
            <span style="margin-left: 8px">{{ row.name }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="size" label="大小" width="120">
          <template #default="{ row }">{{ row.is_dir ? '-' : formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column prop="compressed_size" label="压缩后" width="120">
          <template #default="{ row }">{{ row.is_dir ? '-' : formatSize(row.compressed_size) }}</template>
        </el-table-column>
      </el-table>
    </el-dialog>

    <el-dialog v-model="showApkDialog" :title="`APK 信息: ${apkFileName}`" width="600px">
      <div v-loading="apkLoading">
        <template v-if="apkInfo">
          <el-descriptions :column="1" border>
            <el-descriptions-item label="文件大小">{{ formatSize(apkInfo.file_size) }}</el-descriptions-item>
            <el-descriptions-item label="解压后大小">{{ formatSize(apkInfo.uncompressed_size) }}</el-descriptions-item>
            <el-descriptions-item label="条目数">{{ apkInfo.entry_count }}</el-descriptions-item>
          </el-descriptions>
          <el-divider content-position="left">关键文件</el-divider>
          <el-table :data="apkInfo.key_entries" max-height="300" stripe size="small">
            <el-table-column prop="name" label="名称" min-width="200" />
            <el-table-column prop="size" label="大小" width="100">
              <template #default="{ row }">{{ formatSize(row.size) }}</template>
            </el-table-column>
          </el-table>
        </template>
      </div>
    </el-dialog>
  </el-container>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { filesApi, type FileInfo } from '@/api/files'
import { uploadApi } from '@/api/upload'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Folder, Document, List, Grid, ArrowDown, Upload, Menu, Close, View, Edit, Download } from '@element-plus/icons-vue'

const authStore = useAuthStore()
const currentPath = ref('/')
const fileList = ref<FileInfo[]>([])
const treeData = ref<any[]>([{ name: '根目录', path: '/', has_children: true }])
const viewMode = ref<'list' | 'grid'>('grid')
const searchKeyword = ref('')

const isMobile = ref(false)
const sidebarOpen = ref(false)

function checkMobile() {
  isMobile.value = window.innerWidth <= 768
}

const showMkdirDialog = ref(false)
const newFolderName = ref('')
const showRenameDialogVisible = ref(false)
const renameNewName = ref('')
const renameTarget = ref<FileInfo | null>(null)
const showUploadDialog = ref(false)
const uploadFileList = ref<any[]>([])
const uploading = ref(false)

const showPreviewDialog = ref(false)
const previewType = ref('')
const previewUrl = ref('')
const previewFileName = ref('')
const textContent = ref('')

const showOnlyOfficeDialog = ref(false)
const onlyOfficeFileName = ref('')
const onlyOfficeLoading = ref(false)
let onlyOfficeEditor: any = null
let onlyOfficeScriptLoaded = false

const showArchiveDialog = ref(false)
const archiveFileName = ref('')
const archiveEntries = ref<any[]>([])
const archiveLoading = ref(false)

const showApkDialog = ref(false)
const apkFileName = ref('')
const apkInfo = ref<any>(null)
const apkLoading = ref(false)

const OFFICE_EXTS = ['doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'wps', 'et', 'dps', 'csv', 'rtf', 'odt', 'ods', 'odp', 'txt']
const ARCHIVE_EXTS = ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz']

function getFileExt(name: string): string {
  const dot = name.lastIndexOf('.')
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : ''
}

const treeProps = { label: 'name', children: 'children', isLeaf: (data: any) => !data.has_children }

const breadcrumbs = computed(() => {
  const normalizedPath = currentPath.value.replace(/\\/g, '/')
  const parts = normalizedPath.split('/').filter(Boolean)
  const crumbs = [{ name: '根目录', path: '/' }]
  let path = ''
  for (const part of parts) {
    path += '/' + part
    crumbs.push({ name: part, path })
  }
  return crumbs
})

async function loadFiles() {
  try {
    const res = await filesApi.listFiles({ path: currentPath.value, sort_by: 'type', sort_order: 'asc' })
    fileList.value = res.data.data.items
  } catch (e: any) {
    ElMessage.error('加载文件列表失败: ' + (e.message || ''))
  }
}

async function loadTreeNode(node: any, resolve: Function) {
  const rawPath = node.level === 0 ? '/' : node.data.path
  const path = rawPath.replace(/\\/g, '/')
  try {
    const res = await filesApi.getTree(path)
    resolve(res.data.data || [])
  } catch {
    resolve([])
  }
}

function onTreeNodeClick(data: any) {
  navigateTo(data.path)
  if (isMobile.value) sidebarOpen.value = false
}

async function navigateTo(path: string) {
  currentPath.value = path.replace(/\\/g, '/')
  await loadFiles()
}

function onRowDblClick(row: FileInfo) {
  if (row.is_dir) {
    navigateTo(row.path.replace(/\\/g, '/'))
    return
  }
  const ext = getFileExt(row.name)
  if (ext === 'txt') {
    previewFile(row)
    return
  }
  if (OFFICE_EXTS.includes(ext)) {
    openOnlyOfficeEdit(row)
  } else if (ARCHIVE_EXTS.includes(ext)) {
    previewArchiveFile(row)
  } else if (ext === 'apk') {
    previewApkFile(row)
  } else {
    previewFile(row)
  }
}

function loadOnlyOfficeScript(server: string): Promise<void> {
  return new Promise((resolve, reject) => {
    if (onlyOfficeScriptLoaded && (window as any).DocsAPI) {
      resolve()
      return
    }
    const script = document.createElement('script')
    script.src = `${server}/web-apps/apps/api/documents/api.js`
    script.onload = () => {
      onlyOfficeScriptLoaded = true
      resolve()
    }
    script.onerror = () => reject(new Error('加载 OnlyOffice JS SDK 失败'))
    document.head.appendChild(script)
  })
}

async function openOnlyOfficeEdit(file: FileInfo, mode?: string) {
  onlyOfficeFileName.value = file.name
  onlyOfficeLoading.value = true
  showOnlyOfficeDialog.value = true
  try {
    const res = await filesApi.getOnlyOfficeConfig(file.path, mode)
    const { config, server } = res.data.data
    await loadOnlyOfficeScript(server)
    destroyOnlyOfficeEditor()
    await new Promise(resolve => setTimeout(resolve, 300))

    const dialog = document.querySelector('.onlyoffice-dialog') as HTMLElement
    if (dialog) {
      dialog.style.height = '96vh'
      dialog.style.maxHeight = '96vh'
      const body = dialog.querySelector('.el-dialog__body') as HTMLElement
      if (body) {
        body.style.padding = '0'
        body.style.height = 'calc(96vh - 55px)'
        body.style.overflow = 'hidden'
      }
    }
    const editorDiv = document.getElementById('onlyoffice-editor')
    if (editorDiv) {
      editorDiv.style.width = '100%'
      editorDiv.style.height = '100%'
    }

    onlyOfficeEditor = new (window as any).DocsAPI.DocEditor('onlyoffice-editor', config)
  } catch (e: any) {
    ElMessage.error('打开编辑器失败: ' + (e.message || ''))
    showOnlyOfficeDialog.value = false
  } finally {
    onlyOfficeLoading.value = false
  }
}

function viewOnlyOffice(file: FileInfo) {
  openOnlyOfficeEdit(file, 'view')
}

function destroyOnlyOfficeEditor() {
  if (onlyOfficeEditor) {
    try { onlyOfficeEditor.destroyEditor() } catch {}
    onlyOfficeEditor = null
  }
}

async function previewArchiveFile(file: FileInfo) {
  archiveFileName.value = file.name
  archiveEntries.value = []
  showArchiveDialog.value = true
  archiveLoading.value = true
  try {
    const res = await filesApi.previewArchive(file.path)
    archiveEntries.value = res.data.data.entries
  } catch (e: any) {
    ElMessage.error('读取压缩文件失败: ' + (e.message || ''))
    showArchiveDialog.value = false
  } finally {
    archiveLoading.value = false
  }
}

async function previewApkFile(file: FileInfo) {
  apkFileName.value = file.name
  apkInfo.value = null
  showApkDialog.value = true
  apkLoading.value = true
  try {
    const res = await filesApi.previewApk(file.path)
    apkInfo.value = res.data.data
  } catch (e: any) {
    ElMessage.error('读取APK信息失败: ' + (e.message || ''))
    showApkDialog.value = false
  } finally {
    apkLoading.value = false
  }
}

function onRowContextMenu(_row: FileInfo, _event: MouseEvent) {}

async function doSearch() {
  if (!searchKeyword.value) { await loadFiles(); return }
  try {
    const res = await filesApi.search(searchKeyword.value, currentPath.value)
    fileList.value = res.data.data
  } catch (e: any) {
    ElMessage.error('搜索失败: ' + (e.message || ''))
  }
}

async function createFolder() {
  if (!newFolderName.value) return
  try {
    await filesApi.mkdir(currentPath.value, newFolderName.value)
    ElMessage.success('文件夹创建成功')
    showMkdirDialog.value = false
    newFolderName.value = ''
    await loadFiles()
  } catch (e: any) {
    ElMessage.error(e.message || '创建失败')
  }
}

function showRenameDialog(file: FileInfo) {
  renameTarget.value = file
  renameNewName.value = file.name
  showRenameDialogVisible.value = true
}

async function doRename() {
  if (!renameTarget.value || !renameNewName.value) return
  try {
    await filesApi.rename(renameTarget.value.path, renameNewName.value)
    ElMessage.success('重命名成功')
    showRenameDialogVisible.value = false
    await loadFiles()
  } catch (e: any) {
    ElMessage.error(e.message || '重命名失败')
  }
}

async function confirmDelete(file: FileInfo) {
  try {
    await ElMessageBox.confirm(`确定删除 ${file.name} 吗？`, '确认删除', { type: 'warning' })
    await filesApi.delete(file.path)
    ElMessage.success('删除成功')
    await loadFiles()
  } catch { /* cancelled */ }
}

function downloadFile(file: FileInfo) {
  window.open(filesApi.download(file.path), '_blank')
}

async function previewFile(file: FileInfo) {
  if (!file.preview_type || file.preview_type === 'none') {
    ElMessage.info('不支持预览，请下载查看')
    return
  }
  previewFileName.value = file.name
  previewType.value = file.preview_type
  if (file.preview_type === 'text') {
    try {
      const token = authStore.token
      const resp = await fetch(`/api/files/preview?path=${encodeURIComponent(file.path)}`, {
        headers: { Authorization: `Bearer ${token}` }
      })
      const json = await resp.json()
      textContent.value = json.data?.content || ''
      showPreviewDialog.value = true
    } catch {
      ElMessage.error('预览失败')
    }
  } else {
    previewUrl.value = filesApi.preview(file.path)
    showPreviewDialog.value = true
  }
}

function onFileChange(_file: any, fileListNew: any[]) {
  uploadFileList.value = fileListNew
}

async function startUpload() {
  if (!uploadFileList.value.length) return
  uploading.value = true
  try {
    for (const file of uploadFileList.value) {
      const raw = file.raw as File
      const totalSize = raw.size
      const chunkSize = 5 * 1024 * 1024

      const initRes = await uploadApi.init({
        file_name: raw.name,
        target_path: currentPath.value,
        total_size: totalSize,
      })
      const { task_id, total_chunks } = initRes.data.data
      for (let i = 0; i < total_chunks; i++) {
        const start = i * chunkSize
        const end = Math.min(start + chunkSize, totalSize)
        const chunk = raw.slice(start, end)
        const arrayBuf = await chunk.arrayBuffer()
        await uploadApi.uploadChunk(task_id, i, arrayBuf)
      }
      await uploadApi.complete({
        task_id,
        file_name: raw.name,
        target_path: currentPath.value,
        total_chunks,
      })
    }
    ElMessage.success('上传完成')
    showUploadDialog.value = false
    uploadFileList.value = []
    await loadFiles()
  } catch (e: any) {
    ElMessage.error('上传失败: ' + (e.message || ''))
  } finally {
    uploading.value = false
  }
}

function handleLogout() {
  authStore.logout()
  location.href = '/login'
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i]
}

function formatDate(dateStr: string): string {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  if (isNaN(d.getTime())) return ''
  return `${d.getMonth() + 1}/${d.getDate()}`
}

function getFileIconColor(file: FileInfo): string {
  if (file.is_dir) return '#409eff'
  const ext = getFileExt(file.name)
  const colors: Record<string, string> = {
    pdf: '#f56c6c',
    doc: '#2b579a', docx: '#2b579a', wps: '#2b579a',
    xls: '#217346', xlsx: '#217346', et: '#217346', csv: '#217346',
    ppt: '#d24726', pptx: '#d24726', dps: '#d24726',
    jpg: '#9c27b0', jpeg: '#9c27b0', png: '#9c27b0', gif: '#9c27b0', bmp: '#9c27b0', svg: '#9c27b0',
    mp4: '#ff5722', avi: '#ff5722', mkv: '#ff5722', mov: '#ff5722',
    mp3: '#8bc34a', wav: '#8bc34a', flac: '#8bc34a',
    zip: '#ffc107', rar: '#ffc107', '7z': '#ffc107', tar: '#ffc107', gz: '#ffc107',
    apk: '#009688',
    txt: '#607d8b', rtf: '#607d8b', md: '#607d8b',
  }
  return colors[ext] || '#909399'
}

function getFileIconLabel(file: FileInfo): string {
  if (file.is_dir) return ''
  const ext = getFileExt(file.name)
  if (!ext) return 'FILE'
  return ext.toUpperCase().slice(0, 4)
}

function canPreview(file: FileInfo): boolean {
  return !!file.preview_type && file.preview_type !== 'none'
}

function canEditOffice(file: FileInfo): boolean {
  return OFFICE_EXTS.includes(getFileExt(file.name))
}

onMounted(() => {
  checkMobile()
  window.addEventListener('resize', checkMobile)
  loadFiles()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkMobile)
  destroyOnlyOfficeEditor()
})
</script>

<style scoped>
.file-browser { height: 100vh; }
.toolbar { display: flex; align-items: center; justify-content: space-between; padding: 8px 16px; border-bottom: 1px solid #e4e7ed; flex-wrap: wrap; gap: 8px; }
.toolbar-left { display: flex; align-items: center; gap: 8px; min-width: 0; }
.toolbar-right { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.search-input { width: 200px; }
.menu-toggle { display: none; }
.main-container { flex: 1; min-height: 0; }
.sidebar { width: 240px; border-right: 1px solid #e4e7ed; overflow-y: auto; flex-shrink: 0; }
.sidebar-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #e4e7ed; }
.sidebar-title { font-weight: 600; font-size: 14px; color: #606266; }
.sidebar-close { display: none; }
.sidebar-mask { display: none; }
.file-main { padding: 16px; overflow-y: auto; }
.grid-view { display: flex; flex-wrap: wrap; gap: 16px; padding: 8px; }
.grid-card {
  width: 160px;
  background: #fff;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  padding: 12px;
  cursor: pointer;
  transition: all 0.25s ease;
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.grid-card:hover {
  border-color: #409eff;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  transform: translateY(-2px);
}
.card-icon {
  width: 56px;
  height: 56px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 10px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.12);
}
.card-icon-text {
  color: #fff;
  font-size: 13px;
  font-weight: 700;
  letter-spacing: 1px;
}
.card-info { width: 100%; text-align: center; }
.card-name {
  font-size: 13px;
  color: #303133;
  line-height: 1.4;
  word-break: break-all;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 4px;
  min-height: 18px;
}
.card-meta {
  font-size: 11px;
  color: #909399;
  display: flex;
  justify-content: space-between;
  padding: 0 4px;
}
.card-date { color: #c0c4cc; }
.card-actions {
  position: absolute;
  top: 8px;
  right: 8px;
  display: none;
  gap: 4px;
}
.grid-card:hover .card-actions { display: flex; }
.preview-container { text-align: center; }
.preview-dialog .el-dialog__body { padding: 0; }
.preview-container { padding: 16px; box-sizing: border-box; }
.text-preview { text-align: left; height: calc(100vh - 160px); overflow: auto; background: #f5f7fa; color: #303133; padding: 20px; border-radius: 4px; font-size: 14px; white-space: pre-wrap; word-wrap: break-word; margin: 0; border: 1px solid #e4e7ed; line-height: 1.6; }
.onlyoffice-dialog.el-dialog { max-height: 96vh !important; }
.onlyoffice-dialog .el-dialog__body { padding: 0 !important; height: calc(96vh - 55px) !important; overflow: hidden !important; }
.onlyoffice-container { width: 100%; height: 100%; }
.onlyoffice-editor { width: 100%; height: 100%; }
.onlyoffice-dialog-fullscreen .el-dialog__body { padding: 0 !important; height: calc(100vh - 55px) !important; }
.onlyoffice-dialog-fullscreen .onlyoffice-editor { height: 100%; }

@media (max-width: 768px) {
  .menu-toggle { display: inline-flex; }
  .toolbar { padding: 8px 12px; }
  .search-input { width: 140px; }
  .toolbar-right { gap: 4px; }
  .main-container { position: relative; }
  .sidebar {
    position: fixed;
    left: 0;
    top: 56px;
    bottom: 0;
    z-index: 200;
    width: 240px;
    background: #fff;
    transform: translateX(-100%);
    transition: transform 0.3s ease;
    box-shadow: 2px 0 8px rgba(0,0,0,0.15);
  }
  .sidebar-open { transform: translateX(0); }
  .sidebar-close { display: inline-flex; }
  .sidebar-mask { display: block; position: fixed; left: 0; top: 56px; right: 0; bottom: 0; background: rgba(0,0,0,0.4); z-index: 150; }
  .file-main { padding: 12px; }
  .grid-card { width: 130px; padding: 8px; }
  .onlyoffice-dialog .el-dialog__body { height: calc(100vh - 55px) !important; }
  .onlyoffice-container { height: 100%; }
  .onlyoffice-editor { height: 100%; }
  .text-preview { height: calc(100vh - 100px); }
}

@media (max-width: 480px) {
  .search-input { width: 100px; }
  .toolbar-right .el-button { padding: 8px; }
}
</style>
