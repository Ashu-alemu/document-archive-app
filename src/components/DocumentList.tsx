import type { DocumentRecord } from '../types/document'
import { formatFileSize } from '../services/documentService'

interface Props {
  documents: DocumentRecord[]
  onSelect: (document: DocumentRecord) => void
  onOpen: (document: DocumentRecord) => void
  onToggleFavorite: (document: DocumentRecord) => void
  loading?: boolean
  busy?: boolean
  emptyTitle?: string
  emptyDescription?: string
}

export function DocumentList({ documents, onSelect, onOpen, onToggleFavorite, loading = false, busy = false, emptyTitle = 'No documents found', emptyDescription = 'Try adjusting your search or add the first document to the archive.' }: Props) {
  if (loading) return <div className="empty-state loading-state" role="status" aria-live="polite"><div className="loading-spinner" aria-hidden="true" /><h3>Loading documents…</h3><p>Checking your local archive.</p></div>
  if (!documents.length) return <div className="empty-state"><div className="empty-icon" aria-hidden="true">▱</div><h3>{emptyTitle}</h3><p>{emptyDescription}</p></div>
  return <div className="document-list" aria-live="polite">{documents.map((document) => <article className="document-row" key={document.id}><div className="file-badge" aria-hidden="true">{document.mime_type?.includes('pdf') ? 'PDF' : document.mime_type?.split('/')[0]?.toUpperCase() ?? 'FILE'}</div><div className="document-copy"><h3><button type="button" className="document-title" onClick={() => onSelect(document)}>{document.title}</button></h3><p>{document.document_type} · {new Date(`${document.document_date}T00:00:00`).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}</p><small>{document.person_or_organization || 'No organization'} · {formatFileSize(document.file_size)}</small></div><span className={`status-pill ${document.status}`}>{document.status}</span><button type="button" className="button text-button favorite-button" disabled={busy} aria-label={document.is_favorite ? `Remove ${document.title} from favorites` : `Add ${document.title} to favorites`} aria-pressed={document.is_favorite} onClick={() => onToggleFavorite(document)}>{document.is_favorite ? '★' : '☆'}</button><button type="button" className="button text-button" disabled={busy} onClick={() => onOpen(document)}>Open</button></article>)}</div>
}
