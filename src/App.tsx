import { useEffect, useState } from 'react'
import { testDatabaseConnection } from './database/connection'
import './App.css'

function App() {
  const [databaseStatus, setDatabaseStatus] = useState<'checking' | 'ready' | 'preview'>('checking')

  useEffect(() => { void testDatabaseConnection().then(setDatabaseStatus) }, [])

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-mark">DA</div>
        <div className="brand-copy"><span className="eyebrow">Workspace</span><strong>Document Archive</strong></div>
        <nav aria-label="Primary navigation">
          <a className="nav-item active" href="#overview"><span>⌂</span>Overview</a>
          <a className="nav-item" href="#all-documents"><span>▤</span>All documents</a>
          <a className="nav-item" href="#categories"><span>◈</span>Categories</a>
          <a className="nav-item" href="#settings"><span>⚙</span>Settings</a>
        </nav>
        <div className="sidebar-footer"><span className={`status-dot ${databaseStatus === 'ready' ? 'ready' : ''}`} /><span>{databaseStatus === 'ready' ? 'Local database ready' : 'Foundation preview'}</span></div>
      </aside>
      <main className="main-content" id="overview">
        <header className="topbar"><div><span className="eyebrow">Your records, organized</span><h1>Good morning</h1></div><button className="button primary" type="button">+ Add document</button></header>
        <section className="welcome-card" aria-labelledby="welcome-title"><div><span className="card-kicker">Phase 2 foundation</span><h2 id="welcome-title">A calmer home for important documents.</h2><p>Search, organize, and revisit your organization’s records from one focused desktop workspace.</p></div><div className="welcome-icon" aria-hidden="true">✦</div></section>
        <label className="search-box"><span aria-hidden="true">⌕</span><input type="search" placeholder="Search by title, reference number, organization, or text" /><kbd>⌘ K</kbd></label>
        <section className="stats-grid" aria-label="Archive summary"><article className="stat-card"><span className="stat-label">Documents</span><strong>—</strong><span className="muted">Ready for your first record</span></article><article className="stat-card"><span className="stat-label">This month</span><strong>—</strong><span className="muted">No activity yet</span></article><article className="stat-card"><span className="stat-label">Storage</span><strong>Local</strong><span className="muted">Offline-first by design</span></article></section>
        <section className="recent-section" aria-labelledby="recent-title"><div className="section-heading"><div><span className="eyebrow">Archive activity</span><h2 id="recent-title">Recent documents</h2></div><span className="muted">No documents yet</span></div><div className="empty-state"><div className="empty-icon">▱</div><h3>Your recent documents will appear here</h3><p>When document importing is introduced, this space will show your latest records.</p><button className="button secondary" type="button">Explore foundation</button></div></section>
      </main>
    </div>
  )
}

export default App
