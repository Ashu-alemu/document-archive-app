import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { archiveDocument, deleteDocument, findDocumentByHash, insertDocument, updateDocument } from '../database/documentRepository'
import type { DocumentFormValues, DocumentRecord, SelectedFile } from '../types/document'

interface NativeFileInspection { file_name: string; mime_type: string; file_size: number; file_hash: string }
interface NativeStoredFile { stored_file_name: string; file_path: string; file_size: number; file_hash: string; mime_type: string }
interface NativeExtractedText { text: string | null; status: 'completed' | 'skipped' | 'failed'; available: boolean; message: string | null }

export async function chooseDocumentFile(): Promise<SelectedFile | null> {
  const selected = await open({ multiple: false, directory: false, filters: [{ name: 'Documents', extensions: ['pdf', 'jpg', 'jpeg', 'png', 'webp', 'doc', 'docx', 'txt'] }] })
  if (typeof selected !== 'string') return null
  const inspection = await invoke<NativeFileInspection>('inspect_file', { sourcePath: selected })
  return { sourcePath: selected, fileName: inspection.file_name, mimeType: inspection.mime_type, fileSize: inspection.file_size, fileHash: inspection.file_hash }
}

export async function saveDocument(token: string, values: DocumentFormValues, file: SelectedFile): Promise<DocumentRecord> {
  const title = values.title.trim()
  if (!title || !values.document_date || !values.document_type) throw new Error('Title, document type, and document date are required.')
  const duplicate = await findDocumentByHash(token, file.fileHash)
  if (duplicate) throw new Error(`DUPLICATE:${duplicate.id}`)
  const year = values.document_date.slice(0, 4)
  const month = values.document_date.slice(5, 7)
  const id = `DOC-${year}-${crypto.randomUUID().slice(0, 8).toUpperCase()}`
  const stored = await invoke<NativeStoredFile>('store_file', { token, sourcePath: file.sourcePath, documentId: id, documentType: values.document_type, year, month })
  let extracted: NativeExtractedText
  try {
    extracted = await invoke<NativeExtractedText>('extract_document_text', { token, sourcePath: stored.file_path })
  } catch (error) {
    console.error('OCR failed; preserving the archived document:', error)
    extracted = { text: null, status: 'failed', available: false, message: error instanceof Error ? error.message : String(error) }
  }
  if (!extracted.available && extracted.message) console.warn(extracted.message)
  const now = new Date().toISOString()
  const record: DocumentRecord = {
    id, title, document_type: values.document_type, document_date: values.document_date,
    reference_number: values.reference_number.trim() || null, person_or_organization: values.person_or_organization.trim() || null,
    department: values.department.trim() || null, description: values.description.trim() || null, tags: values.tags.trim() || null,
    file_name: file.fileName, stored_file_name: stored.stored_file_name, file_path: stored.file_path,
    mime_type: stored.mime_type, file_size: stored.file_size, file_hash: stored.file_hash, ocr_text: extracted.text, ocr_status: extracted.status, created_by: null,
    created_at: now, updated_at: now, status: 'active', is_favorite: false,
  }
  try { await insertDocument(token, record) } catch (error) {
    await invoke('remove_stored_file', { token, filePath: stored.file_path }).catch(() => undefined)
    throw error
  }
  return record
}

export async function openDocumentFile(token: string, document: DocumentRecord): Promise<void> {
  try {
    await invoke('open_document', { token, id: document.id })
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    console.error('Failed to open archived file:', error)
    throw new Error(message)
  }
}
export async function archiveDocumentRecord(token: string, document: DocumentRecord): Promise<void> { await archiveDocument(token, document) }
export async function deleteDocumentRecord(token: string, id: string): Promise<void> { await deleteDocument(token, id) }
export async function editDocument(token: string, id: string, values: DocumentFormValues & { status: DocumentRecord['status'] }): Promise<void> { await updateDocument(token, id, values) }

export function formatFileSize(bytes: number | null): string {
  if (bytes === null || bytes === 0) return '—'
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
