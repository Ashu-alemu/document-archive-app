import { useEffect, useMemo, useState } from 'react'
import { listDepartments, selectDepartment } from '../database/departmentRepository'
import type { Department } from '../types/department'

export function DepartmentPortal({ onSelect, onAdministrator }: { onSelect: (department: Department) => void; onAdministrator: () => void }) {
  const [departments, setDepartments] = useState<Department[]>([])
  const [query, setQuery] = useState('')
  const [error, setError] = useState('')
  const filtered = useMemo(() => departments.filter(department => `${department.name} ${department.description}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())), [departments, query])

  useEffect(() => { void listDepartments().then(setDepartments).catch(() => setError('Unable to load departments.')) }, [])
  const choose = async (department: Department) => {
    setError('')
    try { await selectDepartment(department.id); onSelect(department) } catch (reason) { setError(reason instanceof Error ? reason.message : 'Unable to select department.') }
  }

  return <main className="portal-shell"><section className="portal-hero"><div className="portal-brand"><img src="/images/document-archive-logo.png" alt="Church logo" /><div><span className="eyebrow">Church Document Management System</span><h1>Church Archive Portal</h1><p lang="am">ወደ ቤተክርስቲያን ማህደር ፖርታል እንኳን በደህና መጡ</p></div></div><p className="portal-intro">Welcome. Select your department to continue to the secure local archive.</p><label className="portal-search"><span className="sr-only">Search departments</span><span>⌕</span><input value={query} onChange={event => setQuery(event.target.value)} placeholder="Search departments…" /></label></section><section className="portal-content"><div className="portal-heading"><div><span className="eyebrow">Choose a workspace</span><h2>Serve with purpose</h2></div><button type="button" className="button secondary" onClick={onAdministrator}>Administrator login</button></div>{error && <p className="form-error" role="alert">{error}</p>}<div className="department-grid">{filtered.map(department => <button type="button" className="department-card" key={department.id} onClick={() => void choose(department)}><span className="department-icon" aria-hidden="true">
  <img
    src={`/images/departments/${department.id}.png`}
    alt=""
    onError={(event) => {
      event.currentTarget.style.display = 'none'
    }}
  />
</span><span className="department-card-body"><strong>{department.name}</strong><span>{department.description}</span></span><span className="department-arrow" aria-hidden="true">→</span></button>)}</div>{filtered.length === 0 && !error && <p className="portal-empty">No departments match your search.</p>}</section></main>
}
