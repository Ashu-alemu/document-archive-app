import { useEffect, useState, type FormEvent } from 'react'
import { createDepartment, listDepartmentUsers, listManageableDepartments, setDepartmentActive, setDepartmentMembership, updateDepartment, type DepartmentMember } from '../database/departmentRepository'
import type { Department } from '../types/department'

const DEPARTMENT_ROLES = [
  { id: 'department_manager', label: 'Department Manager' },
  { id: 'department_contributor', label: 'Department Contributor' },
  { id: 'department_viewer', label: 'Department Viewer' },
] as const

function message(reason: unknown, fallback: string) { return reason instanceof Error ? reason.message : typeof reason === 'string' ? reason : fallback }

export function DepartmentManagement({ token, globalRole, currentUserId }: { token: string; globalRole: string; currentUserId: string }) {
  const administrator = globalRole === 'admin'
  const [departments, setDepartments] = useState<Department[]>([])
  const [users, setUsers] = useState<DepartmentMember[]>([])
  const [selectedId, setSelectedId] = useState('')
  const [editing, setEditing] = useState<Department | null>(null)
  const [values, setValues] = useState({ name: '', description: '', active: true })
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')

  const loadDepartments = async (resetSelection = false) => {
    setLoading(true); setError('')
    try {
      const next = await listManageableDepartments(token)
      setDepartments(next)
      if (resetSelection || !next.some(department => department.id === selectedId)) setSelectedId(next[0]?.id ?? '')
    } catch (reason) { setError(message(reason, 'Unable to load department management.')) } finally { setLoading(false) }
  }

  const loadUsers = async (departmentId: string) => {
    if (!departmentId) { setUsers([]); return }
    try { setUsers(await listDepartmentUsers(token, departmentId)) } catch (reason) { setError(message(reason, 'Unable to load department users.')) }
  }

  useEffect(() => { void loadDepartments(true) }, [token])
  useEffect(() => { void loadUsers(selectedId) }, [selectedId, token])

  const submit = async (event: FormEvent) => {
    event.preventDefault(); setBusy(true); setError(''); setNotice('')
    try { await createDepartment(token, values.name, values.description, values.active); setValues({ name: '', description: '', active: true }); await loadDepartments(); setNotice('Department created successfully.') } catch (reason) { setError(message(reason, 'Unable to create department.')) } finally { setBusy(false) }
  }

  const saveEdit = async (event: FormEvent) => {
    event.preventDefault(); if (!editing) return; setBusy(true); setError(''); setNotice('')
    try { await updateDepartment(token, editing.id, values.name, values.description); setEditing(null); await loadDepartments(); setNotice('Department updated successfully.') } catch (reason) { setError(message(reason, 'Unable to update department.')) } finally { setBusy(false) }
  }

  const toggle = async (department: Department) => {
    setBusy(true); setError(''); setNotice('')
    try { await setDepartmentActive(token, department.id, department.active !== 1); await loadDepartments(); setNotice(`Department ${department.active ? 'deactivated' : 'activated'} successfully.`) } catch (reason) { setError(message(reason, 'Unable to update department status.')) } finally { setBusy(false) }
  }

  const changeMembership = async (user: DepartmentMember, active: boolean, role = user.departmentRoleId ?? 'department_viewer') => {
    if (!selectedId) return
    setBusy(true); setError(''); setNotice('')
    try { await setDepartmentMembership(token, selectedId, user.id, role, active); await loadUsers(selectedId); setNotice(active ? 'Department membership updated.' : 'User removed from department.') } catch (reason) { setError(message(reason, 'Unable to update membership.')) } finally { setBusy(false) }
  }

  const selected = departments.find(department => department.id === selectedId)
  if (loading) return <section className="settings-panel"><div className="empty-state loading-state" role="status">Loading department management…</div></section>

  return <section className="settings-panel department-management"><div className="section-heading"><div><span className="eyebrow">{administrator ? 'Administrator' : 'Department Manager'}</span><h2>Department roles and memberships</h2><p className="muted">{administrator ? 'Create departments and assign each user a separate role in every department.' : 'Manage memberships only within your assigned department.'}</p></div></div>{error && <p className="form-error" role="alert">{error}</p>}{notice && <p className="notice" role="status">✓ {notice}</p>}{administrator && <form className="department-create-form" onSubmit={editing ? saveEdit : submit}><label>{editing ? 'Edit department name' : 'Department name'}<input required maxLength={80} value={values.name} onChange={event => setValues({ ...values, name: event.target.value })} placeholder="e.g. Outreach" /></label><label>Description<textarea maxLength={240} rows={2} value={values.description} onChange={event => setValues({ ...values, description: event.target.value })} placeholder="Short description for the portal card" /></label>{!editing && <label className="department-active-check"><input type="checkbox" checked={values.active} onChange={event => setValues({ ...values, active: event.target.checked })} /> Active immediately</label>}<div className="form-actions"><button className="button primary" disabled={busy}>{editing ? 'Save changes' : 'Create department'}</button>{editing && <button type="button" className="button secondary" disabled={busy} onClick={() => setEditing(null)}>Cancel</button>}</div></form>}<div className="department-admin-grid"><div className="catalog-panel department-panel"><div className="section-heading"><div><span className="eyebrow">Departments</span><h2>{departments.length} manageable</h2></div></div><div className="department-admin-list">{departments.map(department => <div className={`department-admin-row ${selectedId === department.id ? 'selected' : ''}`} key={department.id}><button type="button" className="department-admin-select" onClick={() => setSelectedId(department.id)}><span className="department-icon" aria-hidden="true">{department.icon}</span><span><strong>{department.name}</strong><small>{department.active ? 'Active' : 'Inactive'} · {department.description || 'No description'}</small></span></button>{administrator && <div className="department-admin-actions"><button type="button" className="button ghost" disabled={busy} onClick={() => { setEditing(department); setValues({ name: department.name, description: department.description, active: department.active === 1 }) }}>Edit</button><button type="button" className="button archive" disabled={busy || department.id === 'administration'} onClick={() => void toggle(department)}>{department.active ? 'Deactivate' : 'Activate'}</button></div>}</div>)}</div></div><div className="catalog-panel department-panel membership-panel"><div className="section-heading"><div><span className="eyebrow">Memberships</span><h2>{selected?.name ?? 'Select a department'}</h2></div></div>{selected ? <><p className="muted">Assign one role per user in this department. Global Administrators retain system-wide access.</p><div className="membership-list">{users.map(user => { const canModify = administrator || (user.globalRoleId !== 'admin' && user.id !== currentUserId); const role = user.departmentRoleId ?? 'department_viewer'; return <div className="membership-row" key={user.id}><input type="checkbox" checked={user.member} disabled={busy || user.active !== 1 || !canModify} onChange={event => void changeMembership(user, event.target.checked, role)} /><span><strong>{user.displayName}</strong><small>{user.username} · {user.globalRoleName}{user.active !== 1 ? ' · Inactive account' : ''}</small></span>{user.member && <select aria-label={`Department role for ${user.displayName}`} value={role} disabled={busy || user.active !== 1 || !canModify} onChange={event => void changeMembership(user, true, event.target.value)}>{DEPARTMENT_ROLES.map(option => <option key={option.id} value={option.id}>{option.label}</option>)}</select>}</div> })}</div>{users.length === 0 && <p className="portal-empty">No users are available.</p>}</> : <p className="portal-empty">Select a department to view and manage its members.</p>}</div></div></section>
}
