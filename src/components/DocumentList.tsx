import type { DocumentRecord } from '../types/document'
import { formatFileSize } from '../services/documentService'

interface Props { documents: DocumentRecord[]; onSelect: (document: DocumentRecord) => void; onOpen: (document: DocumentRecord) => void }
export function DocumentList({ documents, onSelect, onOpen }: Props) {
  if (!documents.length) return <div className="empty-state"><div className="empty-icon">▱</div><h3>No documents found</h3><p>Try adjusting your search or add the first document to the archive.</p></div>
  return <div className="document-list">{documents.map((document) => <article className="document-row" key={document.id} onClick={() => onSelect(document)}><div className="file-badge">{document.mime_type?.includes('pdf') ? 'PDF' : document.mime_type?.split('/')[0]?.toUpperCase() ?? 'FILE'}</div><div className="document-copy"><h3>{document.title}</h3><p>{document.document_type} · {new Date(`${document.document_date}T00:00:00`).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}</p><small>{document.person_or_organization || 'No organization'} · {formatFileSize(document.file_size)}</small></div><span className={`status-pill ${document.status}`}>{document.status}</span><button className="button text-button" onClick={(event) => { event.stopPropagation(); onOpen(document) }}>Open</button></article>)}</div>
}
