import { useAuthStore } from '@/stores/auth'
import api from './index'

export interface FileInfo {
  name: string
  path: string
  is_dir: boolean
  size: number
  modified: string
  mime_type?: string
  preview_type?: string
}

export interface FileListResponse {
  items: FileInfo[]
  total: number
  page: number
  page_size: number
}

export const filesApi = {
  listFiles(params: { path?: string; show_hidden?: boolean; sort_by?: string; sort_order?: string; page?: number; page_size?: number }) {
    return api.get<{ data: FileListResponse }>('/files', { params })
  },
  getTree(path?: string) {
    return api.get<{ data: any[] }>('/files/tree', { params: { path } })
  },
  search(keyword: string, path?: string, maxDepth?: number) {
    return api.get<{ data: FileInfo[] }>('/files/search', { params: { keyword, path, max_depth: maxDepth } })
  },
  mkdir(parentPath: string, name: string) {
    return api.post('/files/mkdir', { parent_path: parentPath, name })
  },
  rename(path: string, newName: string) {
    return api.put('/files/rename', { path, new_name: newName })
  },
  delete(path: string) {
    return api.delete('/files/delete', { data: { path } })
  },
  move(sourcePath: string, targetDir: string) {
    return api.post('/files/move', { source_path: sourcePath, target_dir: targetDir })
  },
  copy(sourcePath: string, targetDir: string) {
    return api.post('/files/copy', { source_path: sourcePath, target_dir: targetDir })
  },
  download(path: string) {
    const authStore = useAuthStore()
    const token = authStore.token || ''
    return `/api/files/download?path=${encodeURIComponent(path)}&token=${encodeURIComponent(token)}`
  },
  batchDownload(paths: string[]) {
    return api.post('/files/download/batch', { paths }, { responseType: 'blob' })
  },
  preview(path: string) {
    const authStore = useAuthStore()
    const token = authStore.token || ''
    return `/api/files/preview?path=${encodeURIComponent(path)}&token=${encodeURIComponent(token)}`
  },
}
