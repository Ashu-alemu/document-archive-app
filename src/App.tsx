import { useCallback, useEffect, useState } from 'react'
import { archiveDocumentRecord, editDocument, openDocumentFile, saveDocument } from './services/documentService'
import { findDocumentById, findDocuments } from './database/documentRepository'
import { DocumentDetails } from './components/DocumentDetails'
import { DocumentForm } from './components/DocumentForm'
import { DocumentList } from './components/DocumentList'
import { SearchBar } from './components/SearchBar'
import type { DocumentFormValues, DocumentRecord, SelectedFile } from './types/document'
import './App.css'

type View = 'dashboard' | 'documents' | 'add' | 'archived' | 'settings' | 'details' | 'edit'
const isDesktop = () => '__TAURI_INTERNALS__' in window

function App() {
  const [view, setView] = useState<View>('dashboard')
  const [documents, setDocuments] = useState<DocumentRecord[]>([])
  const [selected, setSelected] = useState<DocumentRecord | null>(null)
  const [filters, setFilters] = useState({ search: '', type: '', from: '', to: '' })
  const [notice, setNotice] = useState('')
  const [error, setError] = useState('')

  const loadDocuments = useCallback(async () => {
    if (!isDesktop()) return
    try { setDocuments(await findDocuments({ ...filters, status: view === 'archived' ? 'archived' : 'active' })); setError('') } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to load documents.') }
  }, [filters, view])
  // This effect synchronizes the view with the external SQLite data source.
  // oxlint-disable-next-line react(set-state-in-effect)
  useEffect(() => { void loadDocuments() }, [loadDocuments])

  const go = (next: View) => { setNotice(''); setError(''); setSelected(null); setView(next) }
  const saveNew = async (values: DocumentFormValues, file?: SelectedFile) => { if (!file) throw new Error('Select a file before saving.'); const created = await saveDocument(values, file); setNotice(`Document saved successfully. ${created.id}`); setView('documents'); await loadDocuments() }
  const saveEdit = async (values: DocumentFormValues) => { if (!selected) return; await editDocument(selected.id, { ...values, status: selected.status }); setNotice('Metadata updated successfully.'); setSelected(await findDocumentById(selected.id)); setView('details'); await loadDocuments() }
  const open = async (document: DocumentRecord) => { try { await openDocumentFile(document) } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to open this file.') } }
  const archive = async () => { if (!selected) return; await archiveDocumentRecord(selected.id); setNotice('Document archived. The original file was kept.'); go('archived') }
  const heading = view === 'dashboard' ? 'Good morning' : view === 'add' ? 'Add document' : view === 'archived' ? 'Archived documents' : view === 'settings' ? 'Settings' : 'Documents'

  return <div className="app-shell"><aside className="sidebar"><div className="brand-mark">DA</div><div className="brand-copy"><span className="eyebrow">Workspace</span><strong>Document Archive</strong></div><nav aria-label="Primary navigation"><button className={`nav-item ${view === 'dashboard' ? 'active' : ''}`} onClick={() => go('dashboard')}><span>⌂</span>Dashboard</button><button className={`nav-item ${view === 'documents' || view === 'details' || view === 'edit' ? 'active' : ''}`} onClick={() => go('documents')}><span>▤</span>Documents</button><button className={`nav-item ${view === 'add' ? 'active' : ''}`} onClick={() => go('add')}><span>＋</span>Add document</button><button className={`nav-item ${view === 'archived' ? 'active' : ''}`} onClick={() => go('archived')}><span>◫</span>Archived</button><button className={`nav-item ${view === 'settings' ? 'active' : ''}`} onClick={() => go('settings')}><span>⚙</span>Settings</button></nav><div className="sidebar-footer"><span className="status-dot ready" /><span>Offline archive</span></div></aside>
    <main className="main-content"><header className="topbar"><div><span className="eyebrow">Your records, organized</span><h1>{heading}</h1></div>{view !== 'add' && view !== 'settings' && view !== 'edit' && <button className="button primary" onClick={() => go('add')}>+ Add document</button>}</header>{notice && <div className="notice" role="status">✓ {notice}</div>}{error && <div className="form-error" role="alert">{error}</div>}
      {view === 'add' && <DocumentForm onCancel={() => go('documents')} onSave={saveNew} />}
      {view === 'edit' && selected && <DocumentForm initial={selected} onCancel={() => setView('details')} onSave={saveEdit} />}
      {view === 'details' && selected && <DocumentDetails document={selected} onBack={() => go('documents')} onOpen={() => void open(selected)} onEdit={() => setView('edit')} onArchive={() => void archive()} />}
      {view === 'settings' && <div className="empty-state page-empty"><div className="empty-icon">⚙</div><h3>Settings are coming later</h3><p>Phase 3 keeps the archive local and offline-first.</p></div>}
      {view === 'dashboard' && <><section className="welcome-card"><div><span className="card-kicker">Offline-first archive</span><h2>A calmer home for important documents.</h2><p>Your records stay on this computer, with metadata in SQLite and actual files in managed storage.</p></div><div className="welcome-icon" aria-hidden="true">✦</div></section><section className="stats-grid"><article className="stat-card"><span className="stat-label">Active documents</span><strong>{documents.length}</strong><span className="muted">Matching your current archive</span></article><article className="stat-card"><span className="stat-label">Storage</span><strong>Local</strong><span className="muted">Files remain on this device</span></article><article className="stat-card"><span className="stat-label">Phase 3</span><strong>Ready</strong><span className="muted">Search, filter, and open records</span></article></section><section className="recent-section"><div className="section-heading"><div><span className="eyebrow">Archive activity</span><h2>Recent documents</h2></div><button className="button ghost" onClick={() => go('documents')}>View all</button></div><DocumentList documents={documents.slice(0, 5)} onSelect={(document) => { setSelected(document); setView('details') }} onOpen={(document) => void open(document)} /></section></>}
      {(view === 'documents' || view === 'archived') && <><SearchBar {...filters} onChange={setFilters} /><DocumentList documents={documents} onSelect={(document) => { setSelected(document); setView('details') }} onOpen={(document) => void open(document)} /></>}
    </main></div>
}

export default App
