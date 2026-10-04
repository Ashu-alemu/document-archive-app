import { invoke } from '@tauri-apps/api/core'
import type { DocumentFormValues, DocumentRecord, DocumentSearchFilters, DocumentSearchResults, DocumentStatus, ManagedValue } from '../types/document'

export function findDocuments(token: string, options: DocumentSearchFilters & { status?: DocumentStatus | 'all'; limit: number; offset: number }): Promise<DocumentSearchResults> {
  return invoke('find_documents', { token, search: options.search, searchMode: options.searchMode, documentType: options.type, from: options.from, to: options.to, organization: options.organization, tag: options.tag, favoritesOnly: options.favoritesOnly, sortBy: options.sortBy, sortDirection: options.sortDirection, status: options.status ?? 'active', limit: options.limit, offset: options.offset })
}
export function findRecentDocuments(token: string, limit = 5): Promise<DocumentRecord[]> { return invoke('find_recent_documents', { token, limit }) }
export function findDocumentById(token: string, id: string): Promise<DocumentRecord | null> { return invoke('find_document_by_id', { token, id }) }
export function findDocumentByHash(token: string, fileHash: string): Promise<DocumentRecord | null> { return invoke('find_document_by_hash', { token, fileHash }) }
export function insertDocument(token: string, record: DocumentRecord): Promise<void> { return invoke('insert_document', { token, record }) }
export function updateDocument(token: string, id: string, values: DocumentFormValues & { status: DocumentStatus }): Promise<void> { return invoke('update_document', { token, id, values: { title: values.title, documentType: values.document_type, documentDate: values.document_date, referenceNumber: values.reference_number || null, personOrOrganization: values.person_or_organization || null, department: values.department || null, description: values.description || null, tags: values.tags || null, status: values.status } }) }
export function archiveDocument(token: string, record: DocumentRecord): Promise<void> { return updateDocument(token, record.id, { title: record.title, document_type: record.document_type, document_date: record.document_date, reference_number: record.reference_number ?? '', person_or_organization: record.person_or_organization ?? '', department: record.department ?? '', description: record.description ?? '', tags: record.tags ?? '', status: 'archived' }) }
export function deleteDocument(token: string, id: string): Promise<void> { return invoke('delete_document', { token, id }) }
export function setDocumentFavorite(token: string, id: string, favorite: boolean): Promise<void> { return invoke('set_document_favorite', { token, id, favorite }) }
export function listCategories(token: string): Promise<ManagedValue[]> { return invoke('list_categories', { token }) }
export function listTags(token: string): Promise<ManagedValue[]> { return invoke('list_tags', { token }) }
export function createCategory(token: string, name: string): Promise<void> { return invoke('create_category', { token, name }) }
export function renameCategory(token: string, oldName: string, newName: string): Promise<void> { return invoke('rename_category', { token, oldName, newName }) }
export function deleteCategory(token: string, name: string): Promise<void> { return invoke('delete_category', { token, name }) }
export function createTag(token: string, name: string): Promise<void> { return invoke('create_tag', { token, name }) }
export function renameTag(token: string, oldName: string, newName: string): Promise<void> { return invoke('rename_tag', { token, oldName, newName }) }
export function deleteTag(token: string, name: string): Promise<void> { return invoke('delete_tag', { token, name }) }
