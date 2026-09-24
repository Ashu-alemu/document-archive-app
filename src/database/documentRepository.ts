import type { DocumentFormValues, DocumentRecord, DocumentStatus } from '../types/document'
import { openDatabase } from './connection'

export async function findDocuments(options: { search?: string; type?: string; from?: string; to?: string; status?: DocumentStatus | 'all' }): Promise<DocumentRecord[]> {
  const database = await openDatabase()
  const search = (options.search ?? '').trim()
  const term = `%${search}%`
  const type = options.type ?? ''
  const from = options.from ?? ''
  const to = options.to ?? ''
  const status = options.status ?? 'active'
  return database.select<DocumentRecord[]>(
    `SELECT * FROM documents
     WHERE (? = 'all' OR status = ?)
       AND (? = '' OR title LIKE ? OR reference_number LIKE ? OR person_or_organization LIKE ? OR department LIKE ? OR description LIKE ? OR tags LIKE ? OR file_name LIKE ? OR ocr_text LIKE ?)
       AND (? = '' OR document_type = ?)
       AND (? = '' OR document_date >= ?)
       AND (? = '' OR document_date <= ?)
     ORDER BY document_date DESC, created_at DESC`,
    [status, status, search, term, term, term, term, term, term, term, term, type, type, from, from, to, to],
  )
}

export async function findDocumentById(id: string): Promise<DocumentRecord | null> {
  const database = await openDatabase()
  const rows = await database.select<DocumentRecord[]>('SELECT * FROM documents WHERE id = ?', [id])
  return rows[0] ?? null
}

export async function findDocumentByHash(fileHash: string): Promise<DocumentRecord | null> {
  const database = await openDatabase()
  const rows = await database.select<DocumentRecord[]>('SELECT * FROM documents WHERE file_hash = ? LIMIT 1', [fileHash])
  return rows[0] ?? null
}

export async function insertDocument(record: DocumentRecord): Promise<void> {
  const database = await openDatabase()
  await database.execute(
    `INSERT INTO documents (id, title, document_type, document_date, reference_number, person_or_organization, department, description, tags, file_name, stored_file_name, file_path, mime_type, file_size, file_hash, ocr_text, created_by, created_at, updated_at, status)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    [record.id, record.title, record.document_type, record.document_date, record.reference_number, record.person_or_organization, record.department, record.description, record.tags, record.file_name, record.stored_file_name, record.file_path, record.mime_type, record.file_size, record.file_hash, record.ocr_text, record.created_by, record.created_at, record.updated_at, record.status],
  )
}

export async function updateDocument(id: string, values: DocumentFormValues & { status: DocumentStatus }): Promise<void> {
  const database = await openDatabase()
  await database.execute(
    `UPDATE documents SET title = ?, document_type = ?, document_date = ?, reference_number = ?, person_or_organization = ?, department = ?, description = ?, tags = ?, status = ?, updated_at = ? WHERE id = ?`,
    [values.title, values.document_type, values.document_date, values.reference_number || null, values.person_or_organization || null, values.department || null, values.description || null, values.tags || null, values.status, new Date().toISOString(), id],
  )
}

export async function archiveDocument(id: string): Promise<void> {
  const database = await openDatabase()
  await database.execute('UPDATE documents SET status = ?, updated_at = ? WHERE id = ?', ['archived', new Date().toISOString(), id])
}
