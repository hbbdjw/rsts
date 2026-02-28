import { getServiceBaseURL } from '@/utils/service';
import { request } from '../request';

export interface SftpSession {
  session_id: number;
}

export interface FileEntry {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  permissions: number;
  mtime: number;
}

export interface PathQuery {
  session_id: number;
  path?: string;
}

export interface CreateSessionPayload {
  hostname: string;
  port: number;
  username: string;
  password?: string;
}

export interface DeletePayload {
  session_id: number;
  path: string;
}

export interface RenamePayload {
  session_id: number;
  path: string;
  new_name: string;
}

export interface UploadPayload {
  session_id: number;
  path: string;
  filename: string;
  content_base64: string;
}

export interface MkdirPayload {
  session_id: number;
  path: string;
}

export interface WriteFilePayload {
  session_id: number;
  path: string;
  content: string;
}

export interface ChmodPayload {
  session_id: number;
  path: string;
  mode: number;
}

export function createSftpSession(data: CreateSessionPayload) {
  return request<SftpSession>({
    url: '/api/sftp/session',
    method: 'post',
    data
  });
}

export function listFiles(params: PathQuery) {
  return request<FileEntry[]>({
    url: '/api/sftp/list',
    method: 'get',
    params
  });
}

export function readFile(params: PathQuery) {
  return request<{ content: string }>({
    url: '/api/sftp/read',
    method: 'get',
    params
  });
}

export function downloadFile(params: PathQuery) {
  // Use window.open or fetch blob for download
  const queryString = new URLSearchParams({
    session_id: params.session_id.toString(),
    path: params.path || ''
  }).toString();

  const isHttpProxy = import.meta.env.DEV && import.meta.env.VITE_HTTP_PROXY === 'Y';
  const { baseURL } = getServiceBaseURL(import.meta.env, isHttpProxy);

  const finalBaseURL = baseURL.endsWith('/') ? baseURL.slice(0, -1) : baseURL;

  window.open(`${finalBaseURL}/api/sftp/download?${queryString}`, '_blank');
}

export function writeFile(data: { sessionId: number; path: string; content: string }) {
  return request<null>({
    url: '/api/sftp/write',
    method: 'post',
    data: {
      session_id: data.sessionId,
      path: data.path,
      content: data.content
    }
  });
}

export function deleteFile(data: DeletePayload) {
  return request<string>({
    url: '/api/sftp/delete',
    method: 'post',
    data
  });
}

export function renameFile(data: RenamePayload) {
  return request<string>({
    url: '/api/sftp/rename',
    method: 'post',
    data
  });
}

export function uploadFile(data: UploadPayload, onProgress?: (progress: number, speed: string) => void) {
  const startTime = Date.now();

  return request<string>({
    url: '/api/sftp/upload',
    method: 'post',
    data,
    timeout: 120000, // 2 minutes timeout for upload
    onUploadProgress: (progressEvent: any) => {
      if (onProgress && progressEvent.total) {
        const percent = Math.round((progressEvent.loaded * 100) / progressEvent.total);

        const currentTime = Date.now();
        const timeDiff = (currentTime - startTime) / 1000; // in seconds

        let speed = '0 KB/s';
        if (timeDiff > 0) {
          const speedBytes = progressEvent.loaded / timeDiff;
          if (speedBytes > 1024 * 1024) {
            speed = `${(speedBytes / (1024 * 1024)).toFixed(2)} MB/s`;
          } else {
            speed = `${(speedBytes / 1024).toFixed(2)} KB/s`;
          }
        }
        onProgress(percent, speed);
      }
    }
  });
}

export function createDir(data: MkdirPayload) {
  return request<string>({
    url: '/api/sftp/mkdir',
    method: 'post',
    data
  });
}

export function setPermissions(data: ChmodPayload) {
  return request<string>({
    url: '/api/sftp/chmod',
    method: 'post',
    data
  });
}
