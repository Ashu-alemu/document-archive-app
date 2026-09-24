export const DOCUMENT_TYPES = [
  'Letter', 'Receipt', 'Bill / Invoice', 'Meeting', 'Report', 'Certificate',
  'Contract / Agreement', 'Payment Document', 'Photo / Scan', 'Other',
] as const

export type DocumentType = (typeof DOCUMENT_TYPES)[number]
export type DocumentStatus = 'active' | 'archived'
export type OcrStatus = 'completed' | 'skipped' | 'failed'

export interface DocumentRecord {
  id: string
  title: string
  document_type: DocumentType
  document_date: string
  reference_number: string | null
  person_or_organization: string | null
  department: string | null
  description: string | null
  tags: string | null
  file_name: string
  stored_file_name: string
  file_path: string
  mime_type: string | null
  file_size: number | null
  file_hash: string | null
  ocr_text: string | null
  ocr_status: OcrStatus
  created_by: string | null
  created_at: string
  updated_at: string
  status: DocumentStatus
}

export interface DocumentFormValues {
  title: string
  document_type: DocumentType
  document_date: string
  reference_number: string
  person_or_organization: string
  department: string
  description: string
  tags: string
}

export interface SelectedFile {
  sourcePath: string
  fileName: string
  mimeType: string
  fileSize: number
  fileHash: string
}
