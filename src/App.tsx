import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { archiveDocumentRecord, deleteDocumentRecord, editDocument, openDocumentFile, saveDocument } from './services/documentService'
import { findDocumentById, findDocuments, findRecentDocuments, listCategories, listTags, setDocumentFavorite } from './database/documentRepository'
import { DocumentDetails } from './components/DocumentDetails'
import { DocumentForm } from './components/DocumentForm'
import { DocumentList } from './components/DocumentList'
import { SearchBar } from './components/SearchBar'
import { OrganizationManagement } from './components/OrganizationManagement'
import type { DocumentFormValues, DocumentRecord, DocumentSearchFilters, DocumentSearchResults, ManagedValue, SelectedFile } from './types/document'
import './App.css'
import { LoginScreen, LockScreen, SecurityPreferences, SetupScreen, UserManagement } from './components/AuthScreens'
import { AuditLog } from './components/AuditLog'
import { BackupScreen } from './components/BackupScreen'
import { NotificationCenter } from './components/NotificationCenter'
import { hasUsers, logout } from './services/authService'
import { getSecurityPreferences, lockSession } from './database/authRepository'
import type { SessionUser } from './types/auth'

type View = 'dashboard' | 'documents' | 'favorites' | 'add' | 'archived' | 'settings' | 'organization' | 'audit' | 'backup' | 'details' | 'edit'
const isDesktop = () => '__TAURI_INTERNALS__' in window
const PAGE_SIZE = 50
const EMPTY_FILTERS: DocumentSearchFilters = {
  search: '', searchMode: 'all', type: '', from: '', to: '', organization: '', tag: '',
  favoritesOnly: false, sortBy: 'date', sortDirection: 'desc',
}

function App() {
  const [authState, setAuthState] = useState<'loading' | 'setup' | 'login' | 'authenticated' | 'locked'>('loading')
  const [user, setUser] = useState<SessionUser | null>(null)
  const [lockedUser, setLockedUser] = useState<Pick<SessionUser, 'displayName' | 'username'> | null>(null)
  const [timeoutMinutes, setTimeoutMinutes] = useState(5)
  const [view, setView] = useState<View>('dashboard')
  const [results, setResults] = useState<DocumentSearchResults>({ documents: [], total: 0 })
  const [recentDocuments, setRecentDocuments] = useState<DocumentRecord[]>([])
  const [categories, setCategories] = useState<ManagedValue[]>([])
  const [tags, setTags] = useState<ManagedValue[]>([])
  const [selected, setSelected] = useState<DocumentRecord | null>(null)
  const [filters, setFilters] = useState<DocumentSearchFilters>(EMPTY_FILTERS)
  const [page, setPage] = useState(0)
  const [notice, setNotice] = useState('')
  const [error, setError] = useState('')
  const [loadingDocuments, setLoadingDocuments] = useState(false)
  const [actionBusy, setActionBusy] = useState<string | null>(null)
  const [notificationRefreshKey, setNotificationRefreshKey] = useState(0)
  const loadGeneration = useRef(0)
  const lastActivity = useRef(0)
  const timeoutHandle = useRef<number | undefined>(undefined)

  // oxlint-disable-next-line react(set-state-in-effect)
  useEffect(() => { void hasUsers().then(exists => setAuthState(exists ? 'login' : 'setup')).catch(() => setAuthState('login')) }, [])

  const lock = useCallback(async (reason: 'inactivity' | 'suspend') => {
    if (!user || authState !== 'authenticated') return
    const identity = { displayName: user.displayName, username: user.username }
    try { await lockSession(user.token, reason) } catch (error) { if (reason === 'inactivity') setError(error instanceof Error ? error.message : 'Unable to lock the archive.'); return }
    setLockedUser(identity); setUser(null); setAuthState('locked'); setSelected(null); setResults({ documents: [], total: 0 }); setRecentDocuments([]); setCategories([]); setTags([]); setView('dashboard')
  }, [authState, user])

  useEffect(() => {
    if (!user || authState !== 'authenticated') return
    let cancelled = false
    void getSecurityPreferences(user.token).then(preferences => { if (!cancelled) setTimeoutMinutes(preferences.inactivityTimeoutMinutes) }).catch(() => { if (!cancelled) setTimeoutMinutes(5) })
    return () => { cancelled = true }
  }, [authState, user])

  useEffect(() => {
    if (!user || authState !== 'authenticated') return
    const activity = () => { lastActivity.current = Date.now() }
    const events = ['pointerdown', 'pointermove', 'keydown', 'touchstart', 'wheel']
    events.forEach(event => window.addEventListener(event, activity, { passive: true }))
    const tick = () => { const elapsed = Date.now() - lastActivity.current; const delay = timeoutMinutes * 60_000; if (elapsed >= delay) void lock('inactivity'); else timeoutHandle.current = window.setTimeout(tick, delay - elapsed) }
    lastActivity.current = Date.now(); timeoutHandle.current = window.setTimeout(tick, timeoutMinutes * 60_000)
    return () => { events.forEach(event => window.removeEventListener(event, activity)); if (timeoutHandle.current !== undefined) window.clearTimeout(timeoutHandle.current) }
  }, [authState, lock, timeoutMinutes, user])

  useEffect(() => {
    if (!isDesktop()) return
    let active = true
    const unlisten = Promise.all([listen('system-suspend', () => { if (active) void lock('suspend') }), listen('system-resume', () => { if (active && user) void lock('suspend') })])
    return () => { active = false; void unlisten.then(cleanups => cleanups.forEach(cleanup => cleanup())) }
  }, [lock, user])

  const refreshCatalogs = useCallback(async () => {
    if (!user || !isDesktop()) return
    try {
      const [nextCategories, nextTags] = await Promise.all([listCategories(user.token), listTags(user.token)])
      setCategories(nextCategories)
      setTags(nextTags)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to load archive categories and tags.')
    }
  }, [user])
  // Synchronize the catalog view with the authenticated local database.
  // oxlint-disable-next-line react(set-state-in-effect)
  useEffect(() => { void refreshCatalogs() }, [refreshCatalogs])

  const loadDocuments = useCallback(async () => {
    const generation = ++loadGeneration.current
    if (!isDesktop() || !user) return
    const browsing = view === 'dashboard' || view === 'documents' || view === 'favorites' || view === 'archived'
    if (!browsing) return
    const dashboard = view === 'dashboard'
    const activeFilters = dashboard ? { ...EMPTY_FILTERS, sortBy: 'created' as const } : filters
    setLoadingDocuments(true)
    try {
      const [nextResults, recent] = await Promise.all([
        findDocuments(user.token, {
          ...activeFilters,
          favoritesOnly: view === 'favorites' || activeFilters.favoritesOnly,
          status: view === 'archived' ? 'archived' : view === 'favorites' ? 'all' : 'active',
          limit: dashboard ? 5 : PAGE_SIZE,
          offset: dashboard ? 0 : page * PAGE_SIZE,
        }),
        dashboard ? findRecentDocuments(user.token, 5) : Promise.resolve(null),
      ])
      if (generation !== loadGeneration.current) return
      setResults(nextResults)
      if (recent) setRecentDocuments(recent)
      setError('')
    } catch (reason) {
      if (generation !== loadGeneration.current) return
      setError(reason instanceof Error ? reason.message : 'Unable to load documents.')
    } finally {
      if (generation === loadGeneration.current) setLoadingDocuments(false)
    }
  }, [filters, page, user, view])
  // This effect synchronizes the view with the external SQLite data source.
  // Synchronize the current view with the authenticated local database.
  // oxlint-disable-next-line react(set-state-in-effect)
  useEffect(() => { void loadDocuments() }, [loadDocuments])

  if (authState === 'loading') return <main className="auth-shell"><p>Opening local archive…</p></main>
  if (authState === 'setup') return <SetupScreen onComplete={account => { setUser(account); setAuthState('authenticated') }} />
  if (authState === 'login') return <LoginScreen onLogin={account => { setUser(account); setAuthState('authenticated') }} />
  if (authState === 'locked' && lockedUser) return <LockScreen user={lockedUser} onUnlock={account => { setLockedUser(null); setUser(account); setAuthState('authenticated'); setNotice('Archive unlocked successfully.') }} />
  if (!user) return null

  const canAdd = user.role !== 'viewer'
  const canManageUsers = user.role === 'admin'
  const go = (next: View, keepNotice = false) => { if (!keepNotice) setNotice(''); setError(''); setSelected(null); setPage(0); setView(next) }
  const saveNew = async (values: DocumentFormValues, file?: SelectedFile) => {
    if (!file) throw new Error('Select a file before saving.')
    const created = await saveDocument(user.token, values, file)
    setNotice(`Document saved successfully. ${created.id}`)
    setNotificationRefreshKey((key) => key + 1)
    setView('documents')
    setPage(0)
  }
  const saveEdit = async (values: DocumentFormValues) => {
    if (!selected) return
    await editDocument(user.token, selected.id, { ...values, status: selected.status })
    setNotice('Metadata updated successfully.')
    setNotificationRefreshKey((key) => key + 1)
    setSelected(await findDocumentById(user.token, selected.id))
    setView('details')
  }
  const open = async (document: DocumentRecord) => {
    if (actionBusy) return
    setActionBusy(`open:${document.id}`)
    try {
      await openDocumentFile(user.token, document)
      setNotificationRefreshKey((key) => key + 1)
      if (view === 'dashboard') setRecentDocuments(await findRecentDocuments(user.token, 5))
      setNotice(`Opened ${document.title}.`)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to open this file.')
    } finally {
      setActionBusy(null)
    }
  }
  const toggleFavorite = async (document: DocumentRecord) => {
    if (actionBusy) return
    setActionBusy(`favorite:${document.id}`)
    try {
      await setDocumentFavorite(user.token, document.id, !document.is_favorite)
      await loadDocuments()
      setRecentDocuments((current) => current.map((item) => item.id === document.id ? { ...item, is_favorite: !document.is_favorite } : item))
      if (selected?.id === document.id) setSelected({ ...selected, is_favorite: !document.is_favorite })
      setNotice(document.is_favorite ? 'Removed from favorites.' : 'Added to favorites.')
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to update favorite.')
    } finally {
      setActionBusy(null)
    }
  }
  const archive = async () => {
    if (!selected || actionBusy || !window.confirm(`Archive “${selected.title}”? The original file will be kept.`)) return
    setActionBusy(`archive:${selected.id}`)
    try {
      await archiveDocumentRecord(user.token, selected)
      setNotice('Document archived. The original file was kept.')
      setNotificationRefreshKey((key) => key + 1)
      go('archived', true)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to archive this document.')
    } finally {
      setActionBusy(null)
    }
  }
  const remove = async () => {
    if (!selected || actionBusy || !window.confirm(`Delete “${selected.title}” and its stored file permanently?`)) return
    setActionBusy(`delete:${selected.id}`)
    try {
      await deleteDocumentRecord(user.token, selected.id)
      setNotice('Document deleted.')
      go('documents', true)
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Unable to delete this document.')
    } finally {
      setActionBusy(null)
    }
  }
  const updateFilters = (next: DocumentSearchFilters) => { setFilters(next); setPage(0) }
  const refreshAfterOrganizationChange = async () => {
    await refreshCatalogs()
    await loadDocuments()
  }
  const heading = view === 'dashboard' ? 'ሰላም👋 እንኳን ደህና መጡ!'
    : view === 'add' ? 'Add document'
      : view === 'archived' ? 'Archived documents'
        : view === 'favorites' ? 'Favorites'
          : view === 'settings' ? 'Settings'
            : view === 'organization' ? 'Categories & tags'
              : view === 'audit' ? 'Audit logs'
                : view === 'backup' ? 'Backup & restore'
                  : view === 'details' || view === 'edit' ? 'Document details'
                    : 'Documents'
  const selectedDocuments = results.documents
  const totalPages = Math.ceil(results.total / PAGE_SIZE)

  return <div className="app-shell"><aside className="sidebar"><div className="brand-mark sidebar-brand-mark"><img src="/images/document-archive-logo.png" alt="Church logo" /></div><div className="brand-copy"><span className="eyebrow">መልካም የስራ ጊዜ እንዲሆንሎ እንመኛለን</span><strong>ህንፃ አሰሪ ኮሚቴ</strong></div><nav aria-label="Primary navigation">
    <button className={`nav-item ${view === 'dashboard' ? 'active' : ''}`} onClick={() => go('dashboard')}><span>⌂</span>Dashboard</button>
    <button className={`nav-item ${view === 'documents' || view === 'details' || view === 'edit' ? 'active' : ''}`} onClick={() => go('documents')}><span>▤</span>Documents</button>
    <button className={`nav-item ${view === 'favorites' ? 'active' : ''}`} onClick={() => go('favorites')}><span>★</span>Favorites</button>
    {canAdd && <button className={`nav-item ${view === 'add' ? 'active' : ''}`} onClick={() => go('add')}><span>＋</span>Add document</button>}
    <button className={`nav-item ${view === 'archived' ? 'active' : ''}`} onClick={() => go('archived')}><span>◫</span>Archived</button>
    {canManageUsers && <><button className={`nav-item ${view === 'organization' ? 'active' : ''}`} onClick={() => go('organization')}><span>⌘</span>Categories & tags</button><button className={`nav-item ${view === 'audit' ? 'active' : ''}`} onClick={() => go('audit')}><span>☷</span>Audit logs</button><button className={`nav-item ${view === 'backup' ? 'active' : ''}`} onClick={() => go('backup')}><span>⬡</span>Backup & restore</button></>}
    <button className={`nav-item ${view === 'settings' ? 'active' : ''}`} onClick={() => go('settings')}><span>⚙</span>Security</button>
  </nav><div className="sidebar-footer"><span className="status-dot ready" /><span>{user.displayName} · {user.roleName}</span><button className="logout-button" onClick={() => { void logout(user.token); setUser(null); setAuthState('login'); setView('dashboard') }}>Sign out</button></div></aside>
    <main className="main-content"><header className="topbar"><div><span className="eyebrow">አገልግሎታችንን ቅድስት ሥላሴ ይባርኩልን😇</span><h1>{heading}</h1></div><div className="topbar-actions">{canManageUsers && <NotificationCenter token={user.token} refreshKey={notificationRefreshKey} />}{canAdd && !['add', 'settings', 'organization', 'audit', 'edit'].includes(view) && <button className="button primary" onClick={() => go('add')}>+ Add document</button>}</div></header>{notice && <div className="notice" role="status">✓ {notice}</div>}{error && <div className="form-error" role="alert">{error}</div>}
      {view === 'add' && <DocumentForm categories={categories.map(({ name }) => name)} tags={tags.map(({ name }) => name)} onCancel={() => go('documents')} onSave={saveNew} />}
      {view === 'edit' && selected && <DocumentForm initial={selected} categories={categories.map(({ name }) => name)} tags={tags.map(({ name }) => name)} onCancel={() => setView('details')} onSave={saveEdit} />}
      {view === 'details' && selected && <DocumentDetails document={selected} busy={Boolean(actionBusy)} onBack={() => go('documents')} onOpen={() => void open(selected)} onToggleFavorite={() => void toggleFavorite(selected)} onEdit={canAdd ? () => setView('edit') : undefined} onArchive={canAdd ? () => void archive() : undefined} onDelete={user.role === 'admin' || user.role === 'manager' ? () => void remove() : undefined} />}
      {view === 'settings' && <><SecurityPreferences token={user.token} />{canManageUsers && <UserManagement token={user.token} currentUserId={user.id} />}</>}
      {view === 'organization' && canManageUsers && <OrganizationManagement token={user.token} onChanged={() => void refreshAfterOrganizationChange()} />}
      {view === 'audit' && canManageUsers && <AuditLog token={user.token} />}
      {view === 'backup' && canManageUsers && <BackupScreen token={user.token} onRestored={() => { setUser(null); setAuthState('login'); setView('dashboard') }} />}
      {view === 'dashboard' && <><section className="welcome-card"><div><span className="card-kicker">የህንፃ አሰሪ ኮሚቴ</span><h2>የጎተራ 44 ፀበል ፈለገ ዮርዳኖስ ቅድስት ሥላሴ  ወቅድስት አርሴማ ቤተክርስቲያን የህንፃ አሰሪ ኮሚቴ✨</h2><p>”የሚያገለግልም ቢሆን እግዚአብሔር በሚሰጠኝ ሀይል ነው ብሎ ያገልግል” 1ጴጥ 4፡11</p></div><div className="welcome-icon"><img src="/images/document-archive-logo.png" alt="Church logo" /></div></section><section className="stats-grid"><article className="stat-card"><span className="stat-label">Active documents-በስራ ላይ ያሉ ፉይሎች</span><strong>{results.total}</strong><span className="muted">Across your local archive</span></article><article className="stat-card"><span className="stat-label">Storage</span><strong>Local</strong><span className="muted">Files remain on this device</span></article><article className="stat-card"><span className="stat-label">Search</span><strong>Ready</strong><span className="muted">Search metadata and OCR text</span></article></section>      <section className="recent-section"><div className="section-heading"><div><span className="eyebrow">Archive activity</span><h2>Recent documents</h2></div><button type="button" className="button ghost" onClick={() => go('documents')}>Browse all</button></div><DocumentList loading={loadingDocuments} busy={Boolean(actionBusy)} emptyTitle="No documents yet" emptyDescription="Add your first document to start building the archive." documents={recentDocuments.length ? recentDocuments : selectedDocuments} onSelect={(document) => { setSelected(document); setView('details') }} onOpen={(document) => void open(document)} onToggleFavorite={(document) => void toggleFavorite(document)} /></section></>}
      {(view === 'documents' || view === 'favorites' || view === 'archived') && <><SearchBar filters={filters} categories={categories.map(({ name }) => name)} tags={tags.map(({ name }) => name)} onChange={updateFilters} /><p className="result-count" role="status" aria-live="polite">{loadingDocuments ? 'Searching local archive…' : `${results.total} ${results.total === 1 ? 'document' : 'documents'} found`}</p><DocumentList loading={loadingDocuments} busy={Boolean(actionBusy)} documents={selectedDocuments} onSelect={(document) => { setSelected(document); setView('details') }} onOpen={(document) => void open(document)} onToggleFavorite={(document) => void toggleFavorite(document)} />{totalPages > 1 && <div className="audit-pagination"><button type="button" className="button secondary" disabled={page === 0 || loadingDocuments} onClick={() => setPage(page - 1)}>Previous</button><span>Page {page + 1} of {totalPages}</span><button type="button" className="button secondary" disabled={page + 1 >= totalPages || loadingDocuments} onClick={() => setPage(page + 1)}>Next</button></div>}</>}
    </main></div>
}

export default App
